# Copyright (C) The libssh2 project and its contributors.
# SPDX-License-Identifier: BSD-3-Clause

# Keep this list sorted
#
# NOTE: test_auth_keyboard_info_request does not use the network, but when
#       run as Windows standalone test, it intermittently fails on GHA with:
#         `Terminate batch job (Y/N)?`
DOCKER_TESTS = \
  test_aa_warmup \
  test_agent_forward_ok \
  test_auth_keyboard_fail \
  test_auth_keyboard_ok \
  test_auth_password_fail_password \
  test_auth_password_fail_username \
  test_auth_password_ok \
  test_auth_pubkey_fail \
  test_auth_pubkey_ok_dsa \
  test_auth_pubkey_ok_ecdsa \
  test_auth_pubkey_ok_ecdsa_signed \
  test_auth_pubkey_ok_ed25519 \
  test_auth_pubkey_ok_ed25519_encrypted \
  test_auth_pubkey_ok_ed25519_mem \
  test_auth_pubkey_ok_rsa \
  test_auth_pubkey_ok_rsa_aes256gcm \
  test_auth_pubkey_ok_rsa_encrypted \
  test_auth_pubkey_ok_rsa_openssh \
  test_auth_pubkey_ok_rsa_signed \
  test_auth_pubkey_ok_rsa_sha2_256_signed \
  test_hostkey_hash \
  test_read

STANDALONE_TESTS =

SSHD_TESTS = \
  test_ssh2 \
  test_auth_pubkey_ok_ed25519

# Programs of the above that use internal libssh2 functions so they need
# to be statically linked against libssh2
DOCKER_TESTS_STATIC = \
  test_auth_keyboard_info_request \
  test_hostkey

STANDALONE_TESTS_STATIC = \
  test_simple

# Copy of the above for Makefile.am.
# Is there a way to reuse the lists above?
test_auth_keyboard_info_request_LDFLAGS = -static
test_hostkey_LDFLAGS = -static
test_simple_LDFLAGS = -static

librunner_la_SOURCES = \
  runner.c runner.h \
  session_fixture.c session_fixture.h \
  openssh_fixture.c openssh_fixture.h

EXTRA_DIST = \
  CMakeLists.txt \
  key_dsa \
  key_dsa.pub \
  key_dsa_wrong \
  key_dsa_wrong.pub \
  key_ecdsa \
  key_ecdsa.pub \
  key_ecdsa_signed \
  key_ecdsa_signed-cert.pub \
  key_ecdsa_signed.pub \
  key_ed25519 \
  key_ed25519.pub \
  key_ed25519_encrypted \
  key_ed25519_encrypted.pub \
  key_rsa \
  key_rsa.pub \
  key_rsa_aes256gcm \
  key_rsa_aes256gcm.pub \
  key_rsa_encrypted \
  key_rsa_encrypted.pub \
  key_rsa_openssh \
  key_rsa_openssh.pub \
  key_rsa_signed \
  key_rsa_signed-cert.pub \
  key_rsa_signed.pub \
  key_rsa_sha2_256_signed \
  key_rsa_sha2_256_signed-cert.pub \
  key_rsa_sha2_256_signed.pub \
  mansyntax.sh \
  openssh_server/Dockerfile \
  openssh_server/authorized_keys \
  openssh_server/ca_ecdsa \
  openssh_server/ca_ecdsa.pub \
  openssh_server/ca_rsa \
  openssh_server/ca_rsa.pub \
  openssh_server/ca_user_keys.pub \
  openssh_server/ssh_host_ecdsa_key \
  openssh_server/ssh_host_ed25519_key \
  openssh_server/ssh_host_rsa_key \
  openssh_server/sshd_config \
  test_read_algos.test \
  test_read_algos.txt \
  test_sshd.test
