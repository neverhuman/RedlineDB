# Support archive contract

`redline-central-v4.1.0.tar.gz` is reproducible review evidence, not an installable or deployable
composition. It contains the two release-built diagnostic/corpus binaries and the machine-readable
backend contract needed to inspect the reviewed client boundary.

The archive deliberately omits Docker and Compose inputs. The central-service Docker build requires
an exact, separately governed Redline Core source and its native release identity; flattening those
files into this standalone archive would create an unusable and misleading deployment surface.
Deployment must instead use a release-authority composition that binds both repositories by exact
immutable tags, commits, trees, and artifact digests.
