FROM oven/bun:1.3.11 AS build
WORKDIR /app

COPY package.json bun.lock tsconfig.base.json ./
COPY apps/desktop ./apps/desktop
COPY packages/shared ./packages/shared

RUN bun install --frozen-lockfile
RUN bun run build:shared && bun run build:desktop

FROM nginx:1.27-alpine

COPY apps/web/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/apps/desktop/dist /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
