sourcedir = $(shell rpm --eval "%_sourcedir")

srpm: /usr/bin/cargo
	cargo package --target-dir "$(sourcedir)"
	rpmbuild -bs ${spec} --define "_srcrpmdir ${outdir}"

/usr/bin/spectool:
	dnf install -y rpmdevtools

/usr/bin/cargo:
	dnf install -y cargo
