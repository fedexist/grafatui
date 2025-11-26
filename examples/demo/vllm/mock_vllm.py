import time
import random
from prometheus_client import start_http_server, Gauge, Counter, Histogram

# Metrics definition matching vLLM grafana dashboard
# Latency Histograms
E2E_LATENCY = Histogram('vllm:e2e_request_latency_seconds', 'End to end request latency', ['model_name'])
ITL_LATENCY = Histogram('vllm:inter_token_latency_seconds', 'Inter token latency', ['model_name'])
TTFT_LATENCY = Histogram('vllm:time_to_first_token_seconds', 'Time to first token latency', ['model_name'])

# Token Counters
PROMPT_TOKENS = Counter('vllm:prompt_tokens_total', 'Number of prompt tokens processed', ['model_name'])
GEN_TOKENS = Counter('vllm:generation_tokens_total', 'Number of generation tokens processed', ['model_name'])

# Scheduler State Gauges
REQUESTS_RUNNING = Gauge('vllm:num_requests_running', 'Number of requests currently running', ['model_name'])
REQUESTS_WAITING = Gauge('vllm:num_requests_waiting', 'Number of requests waiting to be processed', ['model_name'])
GPU_CACHE_USAGE = Gauge('vllm:gpu_cache_usage_perc', 'GPU KV cache usage percentage', ['model_name'])

MODEL_NAME = "llama-2-7b"

def simulate_metrics():
    while True:
        # Simulate request processing
        # Randomly increment tokens
        if random.random() > 0.1:
            PROMPT_TOKENS.labels(model_name=MODEL_NAME).inc(random.randint(10, 100))
            GEN_TOKENS.labels(model_name=MODEL_NAME).inc(random.randint(1, 10))

        # Simulate latencies
        E2E_LATENCY.labels(model_name=MODEL_NAME).observe(random.uniform(0.1, 2.0))
        ITL_LATENCY.labels(model_name=MODEL_NAME).observe(random.uniform(0.01, 0.1))
        TTFT_LATENCY.labels(model_name=MODEL_NAME).observe(random.uniform(0.05, 0.5))

        # Simulate scheduler state (random walk)
        current_running = REQUESTS_RUNNING.labels(model_name=MODEL_NAME)._value.get()
        new_running = max(0, min(50, current_running + random.randint(-5, 5)))
        REQUESTS_RUNNING.labels(model_name=MODEL_NAME).set(new_running)

        current_waiting = REQUESTS_WAITING.labels(model_name=MODEL_NAME)._value.get()
        new_waiting = max(0, min(20, current_waiting + random.randint(-2, 2)))
        REQUESTS_WAITING.labels(model_name=MODEL_NAME).set(new_waiting)

        # Simulate GPU cache usage
        GPU_CACHE_USAGE.labels(model_name=MODEL_NAME).set(random.uniform(0.4, 0.9))

        time.sleep(0.1)

if __name__ == '__main__':
    # Start up the server to expose the metrics.
    start_http_server(8000)
    print("Mock vLLM metrics server started on port 8000")
    simulate_metrics()
