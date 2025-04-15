ARG TARGETOS=linux
ARG TARGETARCH=x86_64

FROM rustlang/rust:nightly AS chef
RUN apt-get update && apt-get -y upgrade && apt-get install -y --no-install-recommends \
    libclang-dev pkg-config cmake libclang-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGETOS=linux
ARG TARGETARCH=x86_64

ARG TARGET_TRIPLE=${TARGETARCH}-unknown-${TARGETOS}-gnu

ARG FEATURES=""
ENV FEATURES $FEATURES

RUN rustup target add ${TARGET_TRIPLE}

COPY --from=planner /app/recipe.json recipe.json
RUN cargo +nightly chef cook --release --features "$FEATURES" --recipe-path recipe.json --target ${TARGET_TRIPLE}
COPY . .

RUN cargo +nightly build --release --features "$FEATURES" --target ${TARGET_TRIPLE}

FROM alpine AS runtime
ARG TARGETOS=linux
ARG TARGETARCH=x86_64
ARG TARGET_TRIPLE=${TARGETARCH}-unknown-${TARGETOS}-gnu

# Correct the path to include the target triple
COPY --from=builder /app/target/${TARGET_TRIPLE}/release/brontes /usr/local/bin/brontes

EXPOSE 6923
ENTRYPOINT ["/usr/local/bin/brontes"]