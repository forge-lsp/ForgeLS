//! Guest ABI helpers. Packing is frozen here.
//!
//! Guests export `memory`, `forgels_plugin_abi_version() -> i32`,
//! `alloc`, `free`, `lint`, and `format`. `lint` and `format` take
//! `(ptr: i32, len: i32)` and return packed `i64` as
//! `ptr << 32 | len`. Host writes request JSON, reads output JSON,
//! and validates UTF-8. Optional `forgels_plugin_rules() -> i64`
//! returns a JSON array of rule IDs with the same packing.

use wasmtime::{Instance, Memory, Store, StoreLimits};

/// Expected guest ABI version. Anything else is rejected at load.
pub const ABI_VERSION: i32 = 1;

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

/// Reject guests whose ABI version differs from `ABI_VERSION`.
pub fn check_abi_version(
    store: &mut Store<StoreLimits>,
    instance: &Instance,
    path: &str,
) -> Result<(), String> {
    let func = instance
        .get_func(&mut *store, "forgels_plugin_abi_version")
        .ok_or_else(|| func_missing("forgels_plugin_abi_version", path))?;
    let typed = func
        .typed::<(), i32>(&*store)
        .map_err(|err| format!("fls: external plugin {path}: bad abi export: {err}"))?;
    let version = typed
        .call(&mut *store, ())
        .map_err(|err| format!("fls: external plugin {path}: abi check failed: {err}"))?;
    if version != ABI_VERSION {
        return Err(format!(
            "fls: external plugin {path}: unsupported abi {version}, want {ABI_VERSION}"
        ));
    }
    Ok(())
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
}
