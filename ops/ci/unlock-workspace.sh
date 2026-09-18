#!/usr/bin/env bash
# Make leftover self-hosted workdir files deletable before checkout.
# Tests (and previous jobs) may chmod 0444 directories such as readonly
# database fixtures; actions/checkout then fails with EACCES.
#
# Used as the GitHub Actions job_started hook on xbabe runners and as
# the first workflow step. Always exits 0 so a dirty workdir cannot
# fail the job before checkout.
set +e
if [ -n "${GITHUB_WORKSPACE}" ] && [ -d "${GITHUB_WORKSPACE}" ]; then
  chmod -R u+rwx "${GITHUB_WORKSPACE}"
fi
if [ -n "${RUNNER_TOOL_CACHE}" ] && [ -d "${RUNNER_TOOL_CACHE}/redlinedb-target" ]; then
  chmod -R u+rwx "${RUNNER_TOOL_CACHE}/redlinedb-target"
fi
exit 0
