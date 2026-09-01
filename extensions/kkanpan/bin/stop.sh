#!/bin/sh
killall -9 kkanpan 2>/dev/null
killall -CONT cvm 2>/dev/null || true
lipc-set-prop -i com.lab126.powerd preventScreenSaver 0 2>/dev/null || true
lipc-set-prop com.lab126.appmgrd show app://com.lab126.booklet.home 2>/dev/null || true
/usr/sbin/eips -c 2>/dev/null || true
