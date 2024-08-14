# Lan-Bet

A small, for fun-and-not-for-profit, wagering/betting platform for use with my mates at a LAN.

# Build

just requires a `cargo build`, and all the dependencies will be downloaded and built.

# Running

## Server
Requires a [surrealDB](https://surrealdb.com/) instance running, with the namespace `test`, database `lan_bet`. Currently, as this is not ready for production, just uses user: root and pass: root for login.

## Client

Currently using [trunk.rs](https://trunkrs.dev/) to serve the client web application. This will probably be the easiest way to deploy it at the moment
