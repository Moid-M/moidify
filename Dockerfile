FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir --no-compile \
    fastapi>=0.115.0 \
    uvicorn[standard]>=0.32.0 \
    mutagen>=1.47.0 \
    watchdog>=4.0.0 \
    python-multipart>=0.0.9

COPY . .

RUN addgroup --system moidify && adduser --system --ingroup moidify moidify && \
    chown -R moidify:moidify /app

USER moidify

EXPOSE 8000
VOLUME ["/app/music", "/app/covers", "/app/data"]

ENV MOIDIFY_MUSIC_DIR=/app/music
ENV MOIDIFY_COVERS_DIR=/app/covers
ENV MOIDIFY_DB_PATH=/app/data/music.db
ENV MOIDIFY_PORT=8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD python3 -c "import urllib.request; urllib.request.urlopen('http://localhost:$MOIDIFY_PORT/api/home')" || exit 1

CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8000"]
