# Acril - a kitchen-sink Rust actor framework

Acril was created to be a building block for fast, composable, compatible, and correct microservices and actors.

It's minimal by default, but includes most of the basic utilities for building a robust web application,
a type-safe SDK for a REST API, and any other purpose for which you find those utilities useful.

The actor pattern is a very powerful coding style, involving independent "actors", which can receive messages and respond to them.
Using the pattern lets you define concrete boundaries, where responsibilities of one system end, and the responsibilities of another begin.

You have full control over how your actor runs, how it runs, and when it ends.

## Features

This library includes:
- A HTTP client, with traits (and proc-macros to implement those traits) accompanying it, for easy development of SDKs for REST APIs; we use it in our [Alpaca Rust SDK](https://github.com/PassivityTrading/alpaca-rs).
- A HTTP server, (TODO) with a built-in router and optional generation of client endpoints for those routes.
- (soon) A Server-sent Events & WebSockets layers to allow actors to handle events/messages.
- (very soon) A default runtime for actors, accomodating their lifecycle and allowing spawning child tasks and/or actors.
