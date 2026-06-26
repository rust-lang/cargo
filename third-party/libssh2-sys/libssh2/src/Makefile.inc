# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause
CSOURCES =              \
  agent.c               \
  bcrypt_pbkdf.c        \
  channel.c             \
  comp.c                \
  chacha.c              \
  cipher-chachapoly.c   \
  crypt.c               \
  crypto.c              \
  global.c              \
  hostkey.c             \
  keepalive.c           \
  kex.c                 \
  knownhost.c           \
  mac.c                 \
  misc.c                \
  packet.c              \
  pem.c                 \
  poly1305.c            \
  publickey.c           \
  scp.c                 \
  session.c             \
  sftp.c                \
  transport.c           \
  userauth.c            \
  userauth_kbd_packet.c \
  version.c

HHEADERS =              \
  chacha.h              \
  channel.h             \
  cipher-chachapoly.h   \
  comp.h                \
  crypto.h              \
  crypto_config.h       \
  libgcrypt.h           \
  libssh2_priv.h        \
  libssh2_setup.h       \
  mac.h                 \
  mbedtls.h             \
  misc.h                \
  openssl.h             \
  os400qc3.h            \
  packet.h              \
  poly1305.h            \
  session.h             \
  sftp.h                \
  transport.h           \
  userauth.h            \
  userauth_kbd_packet.h \
  wincng.h

EXTRA_DIST =            \
  agent_win.c           \
  blowfish.c            \
  libgcrypt.c           \
  mbedtls.c             \
  openssl.c             \
  os400qc3.c            \
  wincng.c
