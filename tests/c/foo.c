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
