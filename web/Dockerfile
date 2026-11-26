# cicada 管理端生产镜像：Vite 构建 → Nginx 静态托管 + 同源反代。
FROM node:22-alpine AS build
RUN corepack enable
WORKDIR /app
COPY package.json pnpm-lock.yaml .npmrc ./
RUN corepack pnpm@10.34.5 install --frozen-lockfile
COPY . .
RUN corepack pnpm@10.34.5 build

FROM nginx:1.27-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80
