FROM python:3.11-slim

WORKDIR /app

COPY requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt

COPY . .

EXPOSE 8000

VOLUME ["/app/music", "/app/covers", "/app/data"]

ENV MOIDIFY_MUSIC_DIR=/app/music
ENV MOIDIFY_COVERS_DIR=/app/covers
ENV MOIDIFY_DB_PATH=/app/data/music.db
ENV MOIDIFY_PORT=8000

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8000"]
