# Pointers and Thread Safety

Scope: `forgels-rust` skill. Pick the smallest pointer that fits the job.

Source: Apollo full `book/chapter_09.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book
Extra: Mara Bos, Rust Atomics and Locks, https://marabos.nl/atomics/

## Rules

1. Default to `&T` for shared read and `&mut T` for exclusive write. Both are the most common and cheapest.
2. `Box<T>` is for heap ownership: recursive types and large values. `Box` is `Send + Sync` when `T` is.
3. `Rc<T>` is single thread only. `Arc<T>` is the threaded twin. Never send `Rc` or `Cell` across threads.
4. `Cell<T>` is only for `Copy` interior values. `RefCell<T>` allows general interior mutability but panics on double borrow. Both are not `Sync`.
5. `Mutex<T>` and `RwLock<T>` are the threaded mutable holders. Wrap in `Arc` to share. Prefer `RwLock` only when reads clearly dominate.
6. `OnceLock<T>` and `LazyLock<T>` are the threaded one-time cells. Use them for shared config and statics. `OnceCell` and `LazyCell` are the single thread versions.
7. Raw `*const T` and `*mut T` are unsafe and for FFI only. Isolate them behind a safe wrapper with a `// SAFETY:` note.
8. A pointer is thread safe only when its content is. Check `Send + Sync` at the boundary, not after the bug.

Quick map:

- `&T`: shared read. `Send + Sync` when `T` is.
- `&mut T`: exclusive write. Not shared.
- `Box<T>`: owned heap. Good for recursion.
- `Rc<T>`: multi owner, one thread. Not `Send`, not `Sync`.
- `Arc<T>`: multi owner, many threads.
- `Cell<T>`: interior `Copy` only. Not `Sync`.
- `RefCell<T>`: runtime borrow check, may panic. Not `Sync`.
- `Mutex<T>`, `RwLock<T>`: threaded mutability, usually in `Arc`.
- `OnceLock<T>`, `LazyLock<T>`: threaded init once. Good for config.

## Naming

- Name shared owners by role: `shared_config`, `symbol_table`.
- Name lock guards briefly and drop fast: `guard`, `table`.
- Keep `unsafe` blocks tiny with a named safe wrapper: `fn read_raw`.

## Good vs bad

Good (share readonly table across threads):

```rust
use std::sync::Arc;

pub fn spawn_workers(table: Arc<SymbolTable>) -> Vec<std::thread::JoinHandle<usize>> {
    (0..4).map(|worker_id| {
        let owned_table = Arc::clone(&table);
        std::thread::spawn(move || owned_table.len_for(worker_id))
    }).collect()
}
```

Bad (single-thread pointer sent across threads, does not compile for a reason):

```rust
use std::rc::Rc;

pub fn spawn_workers_bad(table: Rc<SymbolTable>) -> Vec<std::thread::JoinHandle<usize>> {
    (0..4).map(|_| {
        let owned_table = Rc::clone(&table);
        std::thread::spawn(move || owned_table.len())
    }).collect()
}
```

Good (recursive node boxed):

```rust
pub enum AstNode {
    Leaf(Span),
    Branch(Box<AstNode>, Box<AstNode>),
}
```

## Checklist

1. Smallest fitting pointer, thread bounds checked.
2. `RefCell` and `Rc` never cross threads.
3. `unsafe` is tiny, wrapped, and justified.
