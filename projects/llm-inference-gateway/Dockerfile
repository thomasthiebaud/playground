# Placeholder — fill this in when you reach exercise 4.1.
#
# Example multi-stage build:
#
#   FROM golang:1.22-alpine AS build
#   WORKDIR /app
#   COPY go.mod go.sum ./
#   RUN go mod download
#   COPY src/ ./src/
#   RUN go build -o /gateway ./src/
#
#   FROM alpine:3.19
#   COPY --from=build /gateway /usr/local/bin/gateway
#   ENTRYPOINT ["gateway"]
