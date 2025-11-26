FROM python:3.9-slim

WORKDIR /app

RUN pip install prometheus_client

COPY mock_vllm.py .

EXPOSE 8000

CMD ["python", "mock_vllm.py"]
