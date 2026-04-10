load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_test")

rust_library(
    name = "hello_lib",
    srcs = ["src/lib.rs"],
    edition = "2021",
    deps = [
        "@crates//:thiserror",
        "@crates//:rand"
    ],
)

rust_binary(
    name = "hello",
    srcs = ["src/main.rs"],
    edition = "2021",
    deps = [":hello_lib"],
)

rust_test(
    name = "hello_test",
    crate = ":hello_lib",
)
