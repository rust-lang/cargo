FROM httpd:2.4-alpine

RUN apk add --no-cache git git-daemon openssl

COPY bar /repos/bar
WORKDIR /repos/bar
RUN git config --global user.email "testuser@example.com" &&\
    git config --global user.name "Test User" &&\
    git init -b master . &&\
    git add Cargo.toml src &&\
    git commit -m "Initial commit" &&\
    mv .git ../bar.git &&\
    cd ../bar.git &&\
    git config --bool core.bare true &&\
    rm -rf ../bar
WORKDIR /

EXPOSE 443

WORKDIR /usr/local/apache2/conf
COPY httpd-cargo.conf .
RUN cat httpd-cargo.conf >> httpd.conf
RUN openssl req -x509 -nodes -days 3650 -newkey rsa:2048 \
    -keyout server.key -out server.crt \
    -subj "/emailAddress=webmaster@example.com/C=US/ST=California/L=San Francisco/O=Rust/OU=Cargo/CN=127.0.0.1"
WORKDIR /
