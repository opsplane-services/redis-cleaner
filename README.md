# Redis Key Cleaner

![build workflow](https://github.com/opsplane-services/redis-cleaner/actions/workflows/docker-build.yml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

This application is used to set the expiry of Redis keys based on a given pattern and time-to-live (TTL) value.

## Requirements

- `cargo 1.66.0` or later
- or `docker`

## Configuration

The application can be configured by command line flags, environment variables and in a yaml formatted configuration file.

### Common line flags

- `--dry-run`: If this flag is set, the application won't set any `TTL` value for those keys, where it is not set, but it will count how many keys will be processed during the operations. (default value: `false`)
- `--config`: Refers to a valid configuration file in yaml format. (default value: `config.yaml`)

### Environment variables

The following environment variables can be set in `.env` (or provided from your environment):

- `REDIS_HOST`: Redis server host.
- `REDIS_PORT`: Redis server port.
- `REDIS_USERNAME`: Username for Redis server.
- `REDIS_PASWORD`: Password for Redis server.
- `REDIS_SCHEME`: Scheme for the Redis server protocol. (default value: `rediss`)
- `NOTIFICATION_WEBHOOK_URL`: If it is set, once cleanup finishes, will send a webhook notification (slack) to this location.
- `NOTIFICATION_CLEANUP_TITLE`: The title in the notification. 
- `NOTIFICATION_TEMPALTE_FILE`: The template file (jinja2) that will be used for generating the notification content. (default value: `notification.j2`)

### config.yaml

- `name`: A reference for the item that will be used in the notification
- `pattern`: The key pattern that will be used during processing the keys.
- `ttlSeconds`: The `TTL` value (in seconds) that will be set for a key if `TTL` value is not set. (-1)
- `batch`: The matched keys are processed in batches. This value how many keys should be processed in one batch.

#### Sample

```yaml
- name: My Custom keys
  pattern: "{my-custom}*"
  ttlSeconds: 86400
  batch: 100000
- name: My Custom another keys
  pattern: "{my-custom-another}*"
  ttlSeconds: 129600
  batch: 100000
```

## Usage

First create a `.env` file and fill its values. (It can be created based on `.env.template`)

```
cargo run -- --dry-run --config config.yaml
```


