FROM node:alpine AS builder

WORKDIR /app
COPY package.json package-lock.json* ./
RUN npm install

COPY . .
RUN npm run build

FROM nginx:alpine

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 80 443

CMD ["/bin/sh", "-c", "[ -f /etc/letsencrypt/live/relay.toanau.com/fullchain.pem ] || printf 'server {\\n  listen 80;\\n  client_max_body_size 1G;\\n  root /usr/share/nginx/html;\\n  index index.html;\\n  location /api/ { proxy_pass http://backend:3000; }\\n  location / { try_files $uri /index.html; }\\n}' > /etc/nginx/conf.d/default.conf; exec nginx -g 'daemon off;'"]
