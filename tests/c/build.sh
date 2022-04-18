#!/bin/bash

gcc -g -c -fPIC foo.c -o foo.o
gcc -g foo.o -shared -o libfoo.so
/opt/onbld/bin/i386/ctfconvert -L VERSION libfoo.so
