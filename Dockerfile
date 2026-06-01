FROM python:3.11-slim AS builder
WORKDIR /build
COPY requirements.txt .
RUN pip install --no-cache-dir --user -r requirements.txt

FROM python:3.11-slim AS runtime
WORKDIR /app
COPY --from=builder /root/.local /root/.local
COPY . .
ENV PATH=/root/.local/bin:$PATH
EXPOSE 8000
VOLUME ["/app/music", "/app/covers", "/app/data"]
ENV MOIDIFY_MUSIC_DIR=/app/music
ENV MOIDIFY_COVERS_DIR=/app/covers
ENV MOIDIFY_DB_PATH=/app/data/music.db
ENV MOIDIFY_PORT=8000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD python3 -c "import urllib.request; urllib.request.urlopen('http://localhost:$MOIDIFY_PORT/api/home')" || exit 1
RUN addgroup --system moidify && adduser --system --ingroup moidify moidify && \
    chown -R moidify:moidify /app
USER moidify
CMD ["uvicorn", "server:app", "--host", "0.0.0.0", "--port", "8000"]
