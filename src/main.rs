extern crate tera;
use clap::Parser;
use dotenv::dotenv;
use log::info;
use redis::Client;
use serde::{Deserialize, Serialize};
use serde_yaml::from_reader;
use std::env;
use std::error::Error;
use tera::{Context, Tera};

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RequestData {
    attachments: Vec<Attachment>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Attachment {
    title: String,
    text: String,
    color: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct CleanupConfig {
    pub name: String,
    pub pattern: String,
    pub ttl_seconds: i64,
    pub batch: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct ProcessingResult {
    config: CleanupConfig,
    processed_keys: i64,
    iterations: i64,
    error_msg: String,
    execution_time: String,
}

#[derive(Parser, Debug)]
#[clap(version = "1.0", author = "Oliver Szabo <oleewere@gmail.com>")]
struct Args {
    #[clap(short, long, default_value = "config.yaml")]
    config: String,
    #[clap(short, long)]
    dry_run: bool,
}

fn render_notification_content(
    file: &str,
    results: Vec<ProcessingResult>,
    tera_glob: &str,
) -> String {
    let tera = Tera::new(tera_glob).unwrap();
    let mut context = Context::new();
    context.insert("results", &results);
    return tera.render(file, &context).unwrap();
}

fn create_redis_client(
    protocol: &str,
    host: &str,
    port: &str,
    username: &str,
    password: &str,
) -> Client {
    let connection_url = format!(
        "{}://{}:{}@{}:{}/",
        protocol, username, password, host, port
    );
    Client::open(connection_url).unwrap()
}

fn expire_keys(
    client: &Client,
    conf: &CleanupConfig,
    dry_run: bool,
) -> (Option<Box<dyn Error>>, i64, i64) {
    let mut connection = client.get_connection().unwrap();
    const LUA_SCRIPT: &str = r###"
	local match = ARGV[1];
	local count = tonumber(ARGV[2]);
	local expire_num = tonumber(ARGV[3]);
	local dry_run = tonumber(ARGV[4]);
	local iterations = 0;
	local max_iterations = 100000;
	local processed = 0;
	local cursor = "0";
	repeat
		iterations = iterations + 1;
		local result = redis.call("SCAN", cursor, "MATCH", match, "COUNT", count);
		for _, v in ipairs(result[2]) do
			local ttl = redis.call("TTL", v)
			if ttl == -1 then
				processed = processed + 1;
				if dry_run == 0 then
        			redis.call("EXPIRE", v, expire_num);
				end
			end
		end
		if iterations < max_iterations then
			cursor = result[1];
		else
			cursor = "0";
		end
	until cursor == "0";
	local ret = {processed, iterations}
	return ret"###;
    let script = redis::Script::new(LUA_SCRIPT);
    let dry_run_num = match dry_run {
        true => 1,
        false => 0,
    };
    let result = script
        .key(conf.pattern.clone())
        .arg(conf.pattern.clone())
        .arg(conf.batch.clone())
        .arg(conf.ttl_seconds.clone())
        .arg(dry_run_num)
        .invoke::<(i64, i64)>(&mut connection);
    let (processed, iterations) = match result {
        Ok(v) => v,
        Err(err) => {
            return (Some(Box::new(err)), 0, 0);
        }
    };
    return (None, processed, iterations);
}

async fn cleanup(client: Client, conf: CleanupConfig, dry_run: bool) -> ProcessingResult {
    let start = std::time::Instant::now();
    let duration = start.elapsed();
    let (error, processed_keys, iterations) = expire_keys(&client, &conf, dry_run);
    ProcessingResult {
        config: conf,
        processed_keys,
        iterations,
        error_msg: error
            .as_ref()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "".to_string()),
        execution_time: format!("{:?}", duration),
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::init();
    let args = Args::parse();
    let redis_host = env::var("REDIS_HOST").unwrap();
    let redis_port = env::var("REDIS_PORT").unwrap();
    let redis_username = env::var("REDIS_USERNAME").unwrap_or("".to_string());
    let redis_password = env::var("REDIS_PASSWORD").unwrap_or("".to_string());
    let redis_protocol = env::var("REDIS_PROTOCOL").unwrap_or("rediss".to_string());
    let webhook_url = env::var("NOTIFICATION_WEBHOOK_URL").unwrap_or("".to_string());
    let cleanup_title =
        env::var("NOTIFICATION_CLEANUP_TITLE").unwrap_or("Redis Cleanup".to_string());
    let notification_template_file =
        env::var("NOTIFICATION_TEMPALTE_FILE").unwrap_or("notification.j2".to_string());
    let config_file = args.config;
    let dry_run = args.dry_run;
    let conf_file = std::fs::File::open(config_file).unwrap();
    let configs: Vec<CleanupConfig> = from_reader(conf_file).unwrap();
    let redis_client = create_redis_client(
        &redis_protocol,
        &redis_host,
        &redis_port,
        &redis_username,
        &redis_password,
    );
    info!("Dry run: {}", dry_run);
    let mut handles = Vec::new();
    let task_count = configs.len();
    for i in 0..task_count {
        let job = tokio::spawn(cleanup(redis_client.clone(), configs[i].clone(), dry_run));
        handles.push(job);
    }
    let mut results = Vec::new();
    for job in handles {
        results.push(job.await.unwrap());
    }
    let mut color = "#2EB67D";
    for res in results.clone() {
        if res.error_msg.trim().is_empty() {
            info!(
                "{} - Number of processed Keys: {}",
                res.config.name, res.processed_keys
            );
            info!("{} - Iterations: {}", res.config.name, res.iterations);
        } else {
            color = "#E01E5A";
            info!(
                "Error setting expire time for keys with name '{}' and match: {} - {}",
                res.config.name, res.config.pattern, res.error_msg
            );
        }
    }
    if !webhook_url.is_empty() {
        let text_content =
            render_notification_content(notification_template_file.as_str(), results, "*.j2");
        let attachment = Attachment {
            text: text_content,
            title: cleanup_title.clone(),
            color: color.to_string(),
        };
        let attachments = vec![attachment];
        let request_data = RequestData {
            attachments: attachments,
        };
        let body = serde_json::to_string(&request_data).unwrap();
        let client = reqwest::Client::new();
        let res = client
            .post(webhook_url)
            .header("Content-type", "application/json")
            .body(body)
            .send()
            .await;
        match res {
            Ok(result) => {
                let status = result.status();
                if status.clone().is_success() {
                    info!("Notification has been sent successfully.");
                } else {
                    let code = status.clone().as_u16();
                    info!(
                        "Notification error: {} - code: {}",
                        result.text().await.unwrap(),
                        code
                    );
                }
            }
            Err(err) => {
                info!("Notification error: {}", err.to_string());
            }
        }
    }
}
