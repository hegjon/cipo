sourcedir = $(shell rpm --eval "%_sourcedir")

srpm: /usr/bin/cargo /usr/lib64/pkgconfig/openssl.pc
	mkdir -p "$(sourcedir)"
	cargo package --target-dir "$(sourcedir)"
	rpmbuild -bs ${spec} --define "_srcrpmdir ${outdir}"

/usr/lib64/pkgconfig/openssl.pc:
	dnf install -y openssl-devel

/usr/bin/cargo:
	dnf install -y cargo
