FROM alpine:3.18

RUN apk add --no-cache openssh git
RUN ssh-keygen -A

RUN addgroup -S testuser && adduser -S testuser -G testuser -s /bin/ash
# NOTE: Ideally the password should be set to *, but I am uncertain how to do
# that in alpine. It shouldn't matter since PermitEmptyPasswords is "no".
RUN passwd -u testuser

RUN mkdir /repos && chown testuser /repos
COPY --chown=testuser:testuser bar /repos/bar
USER testuser
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
USER root

EXPOSE 22

ENTRYPOINT ["/usr/sbin/sshd", "-D", "-E", "/var/log/auth.log"]
