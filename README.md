# ctf-bindgen

A tool for generating Rust bindings from [ctf](https://illumos.org/man/5/ctf).

The `ctf-bindgen` tool allows one to generate Rust bindings for a library that
has been compiled with ctf information, directly from the an ELF library file.

## Usage

Given the following C code.

```c
struct foo {
        int bar;
        float baz;
};

struct foo boo(int x) {
        struct foo f;
        f.bar = x;
        f.baz = 7.0;
        return f;
}
```

Compiled as follows

```bash
gcc -g -c -fPIC foo.c -o foo.o
gcc -g foo.o -shared -o libfoo.so
ctfconvert -L VERSION libfoo.so
```

We can generate Rust bindings with

```bash
ctf-bindgen libfoo.so
```

which results in

```Rust
#[link(name = "foo", kind = "dylib")]
extern "C" {
    pub fn boo(_: u32) -> foo;
}
#[repr(C)]
pub struct foo {
    pub bar: u32,
    pub baz: f32,
}
```

where the above code can be used as follows with an appropriate
`cargo:rustc-link-search=<location of libfoo.so>` in your build.rs file.

```rust
mod foo;

#[test]
fn test_foo() {
    let foo = unsafe{ crate::foo::boo(47) };
    assert_eq!(foo.bar, 47);
    assert_eq!(foo.baz, 7.0);
}
```
