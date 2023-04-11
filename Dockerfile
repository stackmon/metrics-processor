FROM docker.io/rust:latest AS builder

ENV DEBIAN_FRONTEND=noninteractive
RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev
RUN update-ca-certificates

# Create appuser
ENV USER=cloudmon
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"

WORKDIR /cloudmon

# copy over your manifests
COPY ./Cargo.toml ./Cargo.toml
COPY ./src ./src

RUN cargo build --target x86_64-unknown-linux-musl --release -v

##############
## Final image
##############
FROM scratch as metrics-processor

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /cloudmon

# Copy our build
COPY --from=builder /cloudmon/target/x86_64-unknown-linux-musl/release/cloudmon-metrics-convertor ./
COPY --from=builder /cloudmon/target/x86_64-unknown-linux-musl/release/cloudmon-metrics-reporter ./

# Use an unprivileged user.
USER cloudmon:cloudmon

ENV PATH=/cloudmon
CMD ["/cloudmon/cloudmon-metrics-convertor"]
