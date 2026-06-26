# Copyright (C) Alexander Lamaison <alexander.lamaison@gmail.com>
# SPDX-License-Identifier: BSD-3-Clause

FROM debian:testing-slim

RUN apt-get update \
 && apt-get install -y openssh-server \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*
RUN [ -d /var/run/sshd ] || mkdir /var/run/sshd

# Chmodding because, when building on Windows, files are copied in with
# -rwxr-xr-x permissions.
#
# Copying to a temp location, then moving because chmodding the copied file has
# no effect (Docker AUFS-related bug maybe?)

# Host keys
COPY ssh_host_rsa_key /tmp/etc/ssh/ssh_host_rsa_key
RUN install -m 0600 /tmp/etc/ssh/ssh_host_rsa_key /etc/ssh/ssh_host_rsa_key

COPY ssh_host_ecdsa_key /tmp/etc/ssh/ssh_host_ecdsa_key
RUN install -m 0600 /tmp/etc/ssh/ssh_host_ecdsa_key /etc/ssh/ssh_host_ecdsa_key

COPY ssh_host_ed25519_key /tmp/etc/ssh/ssh_host_ed25519_key
RUN install -m 0600 /tmp/etc/ssh/ssh_host_ed25519_key /etc/ssh/ssh_host_ed25519_key

# Trusted CA keys
COPY ca_ecdsa /tmp/etc/ssh/ca_ecdsa
RUN install -m 0600 /tmp/etc/ssh/ca_ecdsa /etc/ssh/ca_ecdsa

COPY ca_rsa /tmp/etc/ssh/ca_rsa
RUN install -m 0600 /tmp/etc/ssh/ca_rsa /etc/ssh/ca_rsa

COPY ca_user_keys.pub /tmp/etc/ssh/ca_user_keys.pub
RUN install -m 0600 /tmp/etc/ssh/ca_user_keys.pub /etc/ssh/ca_user_keys.pub

# User
RUN adduser --disabled-password --gecos 'Test user for libssh2 integration tests' libssh2
RUN echo 'libssh2:my test password' | chpasswd

# Config
RUN sed -i 's/KbdInteractiveAuthentication no/KbdInteractiveAuthentication yes/' /etc/ssh/sshd_config
COPY sshd_config /tmp/etc/ssh/sshd_config
RUN cat /tmp/etc/ssh/sshd_config >> /etc/ssh/sshd_config
RUN echo 'TrustedUserCAKeys /etc/ssh/ca_user_keys.pub' >> /etc/ssh/sshd_config


# SSH login fix. Otherwise user is kicked off after login
RUN sed 's/session\s*required\s*pam_loginuid.so/session optional pam_loginuid.so/g' -i /etc/pam.d/sshd

USER libssh2

RUN mkdir -p /home/libssh2/.ssh
RUN mkdir -p /home/libssh2/sandbox

COPY authorized_keys /tmp/libssh2/.ssh/authorized_keys
RUN install -m 0600 /tmp/libssh2/.ssh/authorized_keys /home/libssh2/.ssh/authorized_keys

USER root

EXPOSE 22
# -e gives logs via 'docker logs'
CMD ["/usr/sbin/sshd", "-D", "-e"]
