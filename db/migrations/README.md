# Migration ownership

Redline Central ships no application tables, so there is no central schema
migration in this candidate. Consumer repositories own forward migrations,
backfill, lock-budget proof, and rollback for their namespaced tables. Adding a
central metadata table requires a reviewed migration and rollback test here.
