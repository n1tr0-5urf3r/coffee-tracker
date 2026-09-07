FROM node:22-trixie-slim
WORKDIR /app
ENV TZ=Europe/Berlin

RUN apt-get update \
    && apt-get install -y --no-install-recommends tzdata \
    && rm -rf /var/lib/apt/lists/*

COPY package.json package-lock.json ./
RUN npm ci

COPY fixtures ./fixtures
COPY index.html vite.config.ts tsconfig*.json ./
COPY public ./public
COPY src ./src
COPY docker-entrypoint.mjs ./docker-entrypoint.mjs

RUN npm run generate:api && npm run build

EXPOSE 3123
CMD ["node", "/app/docker-entrypoint.mjs"]
