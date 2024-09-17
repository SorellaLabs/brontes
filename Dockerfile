ARG TARGETOS=linux
ARG TARGETARCH=x86_64

FROM rustlang/rust:nightly AS chef
RUN apt-get update && apt-get -y upgrade && apt-get install -y libclang-dev pkg-config cmake libclang-dev
RUN cargo install cargo-chef
WORKDIR /app

COPY . .

FROM chef AS planner
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder

ARG FEATURES=""
ENV FEATURES $FEATURES

COPY --from=planner /app/recipe.json recipe.json
RUN cargo +nightly chef cook --release --features "$FEATURES" --recipe-path recipe.json
COPY . .

RUN cargo +nightly build --release --features "$FEATURES"

FROM alpine AS runtime
COPY --from=builder /app/target/release/brontes /usr/local/bin/brontes

EXPOSE 6923
ENTRYPOINT ["/usr/local/bin/brontes"]
