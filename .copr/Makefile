sourcedir = $(shell rpm --eval "%_sourcedir")

srpm: /usr/bin/cargo /usr/lib64/pkgconfig/openssl.pc
	cargo package
	rpmbuild -bs ${spec} --define "_rpmautospec_release_number 0" --define "_sourcedir target/package/" --define "_srcrpmdir ${outdir}"

/usr/lib64/pkgconfig/openssl.pc:
	dnf install -y openssl-devel

/usr/bin/cargo:
	dnf install -y cargo
