FROM ubuntu@sha256:4fbb8e6a8395de5a7550b33509421a2bafbc0aab6c06ba2cef9ebffbc7092d90

COPY rootfs /

ENV LANG=C \
    LC_ALL=C \
    TZ=UTC \
    LD_LIBRARY_PATH=/opt/redline/lib \
    PATH=/opt/redline/bin:/usr/bin:/bin

WORKDIR /evidence
USER 65532:65532
ENTRYPOINT ["/opt/redline/bin/redline-testing"]
