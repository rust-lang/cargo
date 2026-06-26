/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "runner.h"

static const char *EXPECTED_RSA_HOSTKEY =
    "AAAAB3NzaC1yc2EAAAABIwAAAQEArrr/JuJmaZligyfS8vcNur+mWR2ddDQtVdhHzdKU"
    "UoR6/Om6cvxpe61H1YZO1xCpLUBXmkki4HoNtYOpPB2W4V+8U4BDeVBD5crypEOE1+7B"
    "Am99fnEDxYIOZq2/jTP0yQmzCpWYS3COyFmkOL7sfX1wQMeW5zQT2WKcxC6FSWbhDqrB"
    "eNEGi687hJJoJ7YXgY/IdiYW5NcOuqRSWljjGS3dAJsHHWk4nJbhjEDXbPaeduMAwQU9"
    "i6ELfP3r+q6wdu0P4jWaoo3De1aYxnToV/ldXykpipON4NPamsb6Ph2qlJQKypq7J4iQ"
    "gkIIbCU1A31+4ExvcIVoxLQw/aTSbw==";

static const char *EXPECTED_ECDSA_HOSTKEY =
    "AAAAE2VjZHNhLXNoYTItbmlzdHAyNTYAAAAIbmlzdHAyNTYAAABBBC+/syyeKJD9dC2ZH"
    "9Q7iJGReR4YM3rUCMsSynkyXojdfSClGCMY7JvWlt30ESjYvxoTfSRGx6WvaqYK/vPoYQ4=";

static const char *EXPECTED_ED25519_HOSTKEY =
    "AAAAC3NzaC1lZDI1NTE5AAAAIIxtdyg2ZRXE70UwyPVUH3UyfDBV8GX5cPF636P6hjom";

int test(LIBSSH2_SESSION *session)
{
    int rc;
    size_t len;
    int type;
    size_t expected_len = 0;
    char *expected_hostkey = NULL;

    const char *hostkey = libssh2_session_hostkey(session, &len, &type);
    if(!hostkey) {
        print_last_session_error("libssh2_session_hostkey");
        return 1;
    }

    if(type == LIBSSH2_HOSTKEY_TYPE_ED25519) {
        rc = _libssh2_base64_decode(session, &expected_hostkey, &expected_len,
                                    EXPECTED_ED25519_HOSTKEY,
                                    strlen(EXPECTED_ED25519_HOSTKEY));
    }
    else if(type == LIBSSH2_HOSTKEY_TYPE_ECDSA_256) {
        rc = _libssh2_base64_decode(session, &expected_hostkey, &expected_len,
                                    EXPECTED_ECDSA_HOSTKEY,
                                    strlen(EXPECTED_ECDSA_HOSTKEY));
    }
    else if(type == LIBSSH2_HOSTKEY_TYPE_RSA) {
        rc = _libssh2_base64_decode(session, &expected_hostkey, &expected_len,
                                    EXPECTED_RSA_HOSTKEY,
                                    strlen(EXPECTED_RSA_HOSTKEY));
    }
    else {
        fprintf(stderr, "Unexpected type of hostkey: %i\n", type);
        return 1;
    }

    if(rc) {
        print_last_session_error("_libssh2_base64_decode");
        return 1;
    }

    if(len != expected_len) {
        fprintf(stderr, "Hostkey does not have the expected length %ld!=%ld\n",
                (unsigned long)len, (unsigned long)expected_len);
        return 1;
    }

    if(memcmp(hostkey, expected_hostkey, len) != 0) {
        fprintf(stderr, "Hostkeys do not match\n");
        return 1;
    }

    return 0;
}
