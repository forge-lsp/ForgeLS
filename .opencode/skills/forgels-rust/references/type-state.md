# Type State Pattern

Scope: `forgels-rust` skill. Illegal states as compile errors for builders and sessions.

Source: Apollo full `book/chapter_07.md`.
Upstream: https://github.com/apollographql/rust-best-practices/tree/main/book

## Rules

1. Use type state when a wrong call order is a real bug source: builder required fields, connect before send, open before read.
2. Model each state as its own type plus `PhantomData`. Move between states by consuming `self`.
3. Expose only valid methods per state. `build` exists only on the fully set builder. `send` exists only on the connected client.
4. Keep the pattern small. Stop when generics get hard to read or only serve cleverness.
5. Prefer `unreachable!` with a reason only where the type already guarantees the value. Never use it to hide a real `None`.
6. For public builders, consider `bon` instead of hand rolled states when the shape is standard.

## Naming

- State types are past participles or adjectives: `Unset`, `Set`, `Disconnected`, `Connected`.
- Builders consume and return: `fn name(self, value: String) -> Builder<Set, AgeState>`.
- Entry points state the contract: `Builder::new`, `File::open`, `Client::connect`.

## Good vs bad

Good (missing field does not compile):

```rust
use std::marker::PhantomData;

pub struct Unset;
pub struct Set;

pub struct ServerBuilder<Host = Unset, Port = Unset> {
    host: Option<String>,
    port: Option<u16>,
    marker: PhantomData<(Host, Port)>,
}

impl ServerBuilder<Unset, Unset> {
    pub fn new() -> Self {
        Self { host: None, port: None, marker: PhantomData }
    }
}

impl<Port> ServerBuilder<Unset, Port> {
    pub fn host(self, host: String) -> ServerBuilder<Set, Port> {
        ServerBuilder { host: Some(host), port: self.port, marker: PhantomData }
    }
}

impl<Host> ServerBuilder<Host, Unset> {
    pub fn port(self, port: u16) -> ServerBuilder<Host, Set> {
        ServerBuilder { host: self.host, port: Some(port), marker: PhantomData }
    }
}

impl ServerBuilder<Set, Set> {
    pub fn build(self) -> Server {
        Server { host: self.host.unwrap_or_else(|| unreachable!("host is set")), port: self.port.unwrap_or(0) }
    }
}
```

Bad (bool flags checked at runtime):

```rust
pub struct ServerBuilderBad {
    host: Option<String>,
    port: Option<u16>,
}

impl ServerBuilderBad {
    pub fn build(self) -> Server {
        Server { host: self.host.unwrap(), port: self.port.unwrap() }
    }
}
```

## Checklist

1. Illegal call is a compile error, not a runtime check.
2. States are few, names are clear, `PhantomData` is the only overhead.
3. `unreachable!` appears only where types already prove presence.
