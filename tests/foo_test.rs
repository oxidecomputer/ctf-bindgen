// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

mod foo;

#[test]
fn test_foo() {
    let foo = unsafe { crate::foo::boo(47) };
    assert_eq!(foo.bar, 47);
    assert_eq!(foo.baz, 7.0);
}
