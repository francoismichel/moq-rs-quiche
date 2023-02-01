FROM golang

WORKDIR /usr/src/warp

ENV GOPRIVATE=*

COPY . .
RUN go build -v -o /usr/local/bin/warp-server .

CMD [ "warp-server" ]
