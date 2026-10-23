//! Guest ABI helpers. Packing is frozen here.
//!
//! Guests export `memory`, `forgels_plugin_abi_version() -> i32`,
//! `forgels_plugin_abi_minor() -> i32`, `alloc`, `free`, `lint`,
//! and `format`. `lint` and `format` take `(ptr: i32, len: i32)`
//! and return packed `i64` as `ptr << 32 | len`. Host writes
//! request JSON, reads output JSON, and validates UTF-8.
//! Optional `forgels_plugin_rules() -> i64` returns a JSON array
//! of rule IDs with the same packing. Minor export is optional:
//! absent means 0 for guests built before the minor split.

use wasmtime::{Instance, Memory, Store, StoreLimits};

/// Guest ABI major. Mismatch rejects the guest at load.
/// Guest ABI minor. Ahead warns once, exact or behind is silent.
pub const ABI_MAJOR: i32 = 1;
pub const ABI_MINOR: i32 = 0;
/// Alias for the major line. Kept for callers on the old name.
pub const ABI_VERSION: i32 = ABI_MAJOR;

/// Compat outcome for a guest. Pure value, no side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbiCompat {
    Silent,
    Warn { major: i32, minor: i32 },
}

/// Pack `(ptr, len)` into one `i64`. Frozen: guests and hosts share it.
#[allow(dead_code)]
pub fn pack_ptr_len(ptr: u32, len: u32) -> i64 {
    (((ptr as u64) << 32) | (len as u64)) as i64
}

/// Unpack one `i64` into `(ptr, len)`. Inverse of `pack_ptr_len`.
pub fn unpack_ptr_len(packed: i64) -> (u32, u32) {
    let raw = packed as u64;
    ((raw >> 32) as u32, (raw & 0xFFFF_FFFF) as u32)
}

fn func_missing(name: &str, path: &str) -> String {
    format!("fls: external plugin {path}: missing export {name}")
}

fn memory_of(
    instance: &Instance,
    store: &mut Store<StoreLimits>,
    path: &str,
) -> Result<Memory, String> {
    instance
        .get_memory(&mut *store, "memory")
        .ok_or_else(|| func_missing("memory", path))
}

fn read_abi_major(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<i32, String> {
    let func = instance
        .get_func(&mut *store, "forgels_plugin_abi_version")
        .ok_or_else(|| func_missing("forgels_plugin_abi_version", path))?;
    let typed = func
        .typed::<(), i32>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad abi export: {err}"))?;
    typed
        .call(&mut *store, ())
        .map_err(|err| format!("fls: external plugin {path}: abi check failed: {err}"))
}

/// Read guest minor. Absent export means 0 for older guests.
pub fn read_abi_minor(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<i32, String> {
    let Some(func) = instance.get_func(&mut *store, "forgels_plugin_abi_minor") else {
        return Ok(0);
    };
    let typed = func
        .typed::<(), i32>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad abi export: {err}"))?;
    typed
        .call(&mut *store, ())
        .map_err(|err| format!("fls: external plugin {path}: abi check failed: {err}"))
}

fn major_reject(path: &str, major: i32) -> String {
    format!("fls: external plugin {path}: unsupported abi {major}.0, want {ABI_VERSION}.x")
}

/// Reject guests whose major differs from `ABI_MAJOR`.
#[allow(dead_code)]
pub fn check_abi_version(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<(), String> {
    let version = read_abi_major(store, instance, path)?;
    if version != ABI_MAJOR {
        return Err(major_reject(path, version));
    }
    Ok(())
}

/// Major decides load, minor decides warn. Pure, no printing.
pub fn check_abi_compat(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<AbiCompat, String> {
    let major = read_abi_major(store, instance, path)?;
    if major != ABI_MAJOR {
        return Err(major_reject(path, major));
    }
    let minor = read_abi_minor(store, instance, path)?;
    if minor > ABI_MINOR {
        return Ok(AbiCompat::Warn { major, minor });
    }
    Ok(AbiCompat::Silent)
}

/// Write `bytes` into guest memory via `alloc`. Returns `(ptr, len)`.
pub fn write_input(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
    bytes: &[u8],
) -> Result<(u32, u32), String> {
    if bytes.len() > i32::MAX as usize {
        return Err(format!("fls: external plugin {path}: request too large"));
    }
    let len = bytes.len() as u32;
    let alloc = instance
        .get_func(&mut *store, "alloc")
        .ok_or_else(|| func_missing("alloc", path))?;
    let alloc = alloc
        .typed::<i32, i32>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad alloc export: {err}"))?;
    let ptr = alloc
        .call(&mut *store, len as i32)
        .map_err(|err| format!("fls: external plugin {path}: alloc failed: {err}"))?;
    if ptr < 0 {
        return Err(format!("fls: external plugin {path}: alloc returned {ptr}"));
    }
    let memory = memory_of(instance, store, path)?;
    memory
        .write(&mut *store, ptr as usize, bytes)
        .map_err(|err| format!("fls: external plugin {path}: memory write failed: {err}"))?;
    Ok((ptr as u32, len))
}

/// Read `packed` output from guest memory. Validates bounds only.
pub fn read_output(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
    packed: i64,
) -> Result<Vec<u8>, String> {
    let (ptr, len) = unpack_ptr_len(packed);
    if len as usize > 16 * 1024 * 1024 {
        return Err(format!("fls: external plugin {path}: output too large"));
    }
    let memory = memory_of(instance, store, path)?;
    let mut buf = vec![0u8; len as usize];
    memory
        .read(&*store, ptr as usize, &mut buf)
        .map_err(|err| format!("fls: external plugin {path}: memory read failed: {err}"))?;
    Ok(buf)
}

/// Free a guest allocation via `free`. Best effort: errors propagate.
pub fn free_guest(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
    ptr: u32,
    len: u32,
) -> Result<(), String> {
    let Some(func) = instance.get_func(&mut *store, "free") else {
        return Ok(());
    };
    let typed = func
        .typed::<(i32, i32), ()>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad free export: {err}"))?;
    typed
        .call(&mut *store, (ptr as i32, len as i32))
        .map_err(|err| format!("fls: external plugin {path}: free failed: {err}"))?;
    Ok(())
}

/// Call guest `lint(ptr, len) -> i64` and return output as UTF-8 text.
pub fn call_lint(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
    request_json: &str,
) -> Result<String, String> {
    let (ptr, len) = write_input(store, instance, path, request_json.as_bytes())?;
    let func = instance
        .get_func(&mut *store, "lint")
        .ok_or_else(|| func_missing("lint", path))?;
    let typed = func
        .typed::<(i32, i32), i64>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad lint export: {err}"))?;
    let packed = typed
        .call(&mut *store, (ptr as i32, len as i32))
        .map_err(|err| format!("fls: external plugin {path}: lint call failed: {err}"))?;
    let bytes = read_output(store, instance, path, packed)?;
    let (out_ptr, out_len) = unpack_ptr_len(packed);
    let _ = free_guest(store, instance, path, ptr, len);
    let _ = free_guest(store, instance, path, out_ptr, out_len);
    String::from_utf8(bytes)
        .map_err(|_| format!("fls: external plugin {path}: lint output not utf-8"))
}

/// Call guest `format(ptr, len) -> i64` and return output as UTF-8 text.
pub fn call_format(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
    request_json: &str,
) -> Result<String, String> {
    let (ptr, len) = write_input(store, instance, path, request_json.as_bytes())?;
    let func = instance
        .get_func(&mut *store, "format")
        .ok_or_else(|| func_missing("format", path))?;
    let typed = func
        .typed::<(i32, i32), i64>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad format export: {err}"))?;
    let packed = typed
        .call(&mut *store, (ptr as i32, len as i32))
        .map_err(|err| format!("fls: external plugin {path}: format call failed: {err}"))?;
    let bytes = read_output(store, instance, path, packed)?;
    let (out_ptr, out_len) = unpack_ptr_len(packed);
    let _ = free_guest(store, instance, path, ptr, len);
    let _ = free_guest(store, instance, path, out_ptr, out_len);
    String::from_utf8(bytes)
        .map_err(|_| format!("fls: external plugin {path}: format output not utf-8"))
}

/// Query optional `forgels_plugin_rules() -> i64` for rule IDs.
/// Missing export means no declared rules. Output must be a JSON
/// array of strings; entries are returned raw for shape checks.
pub fn call_rules(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<Vec<String>, String> {
    let Some(func) = instance.get_func(&mut *store, "forgels_plugin_rules") else {
        return Ok(Vec::new());
    };
    let typed = func
        .typed::<(), i64>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad rules export: {err}"))?;
    let packed = typed
        .call(&mut *store, ())
        .map_err(|err| format!("fls: external plugin {path}: rules call failed: {err}"))?;
    let bytes = read_output(store, instance, path, packed)?;
    let (out_ptr, out_len) = unpack_ptr_len(packed);
    let _ = free_guest(store, instance, path, out_ptr, out_len);
    let text = String::from_utf8(bytes)
        .map_err(|_| format!("fls: external plugin {path}: rules output not utf-8"))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("fls: external plugin {path}: bad rules json: {err}"))?;
    let list = value
        .as_array()
        .ok_or_else(|| format!("fls: external plugin {path}: rules must be an array"))?;
    let mut out = Vec::new();
    for entry in list {
        let Some(name) = entry.as_str() else {
            return Err(format!(
                "fls: external plugin {path}: rules must be strings"
            ));
        };
        out.push(name.to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::{Engine, Module, StoreLimitsBuilder};

    fn instance_for(wat_text: &str) -> (Store<StoreLimits>, Instance) {
        let engine = Engine::default();
        let bytes = wat::parse_str(wat_text).expect("wat parses");
        let module = Module::new(&engine, &bytes).expect("module compiles");
        let limits = StoreLimitsBuilder::new()
            .memory_size(64 * 1024 * 1024)
            .build();
        let mut store = Store::new(&engine, limits);
        store.limiter(|limits| limits as &mut dyn wasmtime::ResourceLimiter);
        let instance = Instance::new(&mut store, &module, &[]).expect("instantiates");
        (store, instance)
    }

    fn wat_for(major: i32, minor: Option<i32>) -> String {
        let minor_func = minor.map_or(String::new(), |value| {
            format!(r#" (func (export "forgels_plugin_abi_minor") (result i32) i32.const {value})"#)
        });
        format!(
            r#"(module (memory (export "memory") 1) (func (export "forgels_plugin_abi_version") (result i32) i32.const {major}){minor_func})"#
        )
    }

    #[test]
    fn packing_round_trips_ptr_and_len() {
        for (ptr, len) in [(0, 0), (1, 2), (1024, 256), (u32::MAX, u32::MAX)] {
            assert_eq!(unpack_ptr_len(pack_ptr_len(ptr, len)), (ptr, len));
        }
    }

    #[test]
    fn packing_freezes_high_low_layout() {
        let packed = pack_ptr_len(0x0102_0304, 0x0506_0708);
        assert_eq!(packed, 0x0102_0304_0506_0708u64 as i64);
    }

    #[test]
    fn major_mismatch_rejects_with_dotted_versions() {
        let (mut store, instance) = instance_for(&wat_for(2, Some(0)));
        let err = check_abi_version(&mut store, &instance, "abi-test").expect_err("rejects");
        assert!(err.contains("unsupported abi"), "keeps prefix: {err}");
        assert!(err.contains("2.0"), "names guest dotted: {err}");
        assert!(err.contains("1."), "names host dotted: {err}");
        let err = check_abi_compat(&mut store, &instance, "abi-test").expect_err("rejects");
        assert!(err.contains("2.0"), "compat names guest: {err}");
        assert!(err.contains("1."), "compat names host: {err}");
    }

    #[test]
    fn minor_ahead_warns() {
        let (mut store, instance) = instance_for(&wat_for(1, Some(1)));
        assert_eq!(
            check_abi_compat(&mut store, &instance, "abi-test"),
            Ok(AbiCompat::Warn { major: 1, minor: 1 })
        );
        assert!(check_abi_version(&mut store, &instance, "abi-test").is_ok());
    }

    #[test]
    fn exact_minor_stays_silent() {
        let (mut store, instance) = instance_for(&wat_for(1, Some(0)));
        assert_eq!(
            check_abi_compat(&mut store, &instance, "abi-test"),
            Ok(AbiCompat::Silent)
        );
    }

    #[test]
    fn missing_minor_defaults_to_zero_silent() {
        let (mut store, instance) = instance_for(&wat_for(1, None));
        assert_eq!(read_abi_minor(&mut store, &instance, "abi-test"), Ok(0));
        assert_eq!(
            check_abi_compat(&mut store, &instance, "abi-test"),
            Ok(AbiCompat::Silent)
        );
    }
}
