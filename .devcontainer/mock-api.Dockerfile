# First build the server binary from source ...
FROM rust:1.95-alpine AS builder
COPY .. /workspace
WORKDIR /workspace
RUN cargo build --package amos-api-server --release

# ... then copy it to a fresh container ready to run
FROM alpine
COPY --from=builder /workspace/target/release/amos-api-server /server
USER 1000
ENTRYPOINT ["/server"]