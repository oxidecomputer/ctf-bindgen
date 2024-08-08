// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
