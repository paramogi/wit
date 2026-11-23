.POSIX:

PREFIX = /usr/local
DOCPREFIX = ${PREFIX}/share/doc/wit
DOC = style.css README.md LICENSE

all:
	cargo build --release --locked

install:
	mkdir -p ${DESTDIR}${PREFIX}/bin
	cp -f target/release/wit ${DESTDIR}${PREFIX}/bin
	chmod 755 ${DESTDIR}${PREFIX}/bin/wit
	mkdir -p ${DESTDIR}${DOCPREFIX}/fonts
	cp -f ${DOC} contrib/post-receive ${DESTDIR}${DOCPREFIX}
	cp -f fonts/* ${DESTDIR}${DOCPREFIX}/fonts

uninstall:
	rm -f ${DESTDIR}${PREFIX}/bin/wit
	rm -rf ${DESTDIR}${DOCPREFIX}

clean:
	cargo clean

.PHONY: all install uninstall clean
