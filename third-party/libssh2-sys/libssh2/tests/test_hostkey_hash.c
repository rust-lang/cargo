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

#if LIBSSH2_MD5
static const char *EXPECTED_RSA_MD5_HASH_DIGEST =
    "0C0ED1A5BB10275F76924CE187CE5C5E";
#endif

static const char *EXPECTED_RSA_SHA1_HASH_DIGEST =
    "F3CD59E2913F4422B80F7B0A82B2B89EAE449387";

static const char *EXPECTED_RSA_SHA256_HASH_DIGEST =
    "92E3DA49DF3C7F99A828F505ED8239397A5D1F62914459760F878F7510F563A3";

#if LIBSSH2_MD5
static const char *EXPECTED_ECDSA_MD5_HASH_DIGEST =
    "0402E4D897580BBC911379CBD88BCD3D";
#endif

static const char *EXPECTED_ECDSA_SHA1_HASH_DIGEST =
    "12FDAD1E3B31B10BABB00F2A8D1B9A62C326BD2F";

static const char *EXPECTED_ECDSA_SHA256_HASH_DIGEST =
    "56FCD975B166C3F0342D0036E44C311A86C0EAE40713B53FC776369BAE7F5264";

static const char *EXPECTED_ED25519_SHA256_HASH_DIGEST =
    "2638B020F6121FA750A7F4754B718419F621814C6E779D68ADF26AA68814ADDF";

#if LIBSSH2_MD5
static const size_t MD5_HASH_SIZE = 16;
#endif
static const size_t SHA1_HASH_SIZE = 20;
static const size_t SHA256_HASH_SIZE = 32;

static void calculate_digest(const char *hash, size_t hash_len, char *buffer,
                             size_t buffer_len)
{
    size_t i;
    char *p = buffer;
    char *end = buffer + buffer_len;

    for(i = 0; i < hash_len && p < end; ++i) {
        p += snprintf(p, (size_t)(end - p), "%02X", (unsigned char)hash[i]);
    }
}

int test(LIBSSH2_SESSION *session)
{
    char buf[BUFSIZ];

    const char *hostkey;
#if LIBSSH2_MD5
    const char *md5_hash;
#endif
    const char *sha1_hash;
    const char *sha256_hash;
    int type;
    size_t len;

    /* these are the host keys under test, they are currently unused */
    (void)EXPECTED_RSA_HOSTKEY;
    (void)EXPECTED_ECDSA_HOSTKEY;
    (void)EXPECTED_ED25519_HOSTKEY;

    hostkey = libssh2_session_hostkey(session, &len, &type);
    if(!hostkey) {
        print_last_session_error("libssh2_session_hostkey");
        return 1;
    }

    if(type == LIBSSH2_HOSTKEY_TYPE_ED25519) {

        sha256_hash = libssh2_hostkey_hash(session,
                                           LIBSSH2_HOSTKEY_HASH_SHA256);
        if(!sha256_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_SHA256)");
            return 1;
        }

        calculate_digest(sha256_hash, SHA256_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_ED25519_SHA256_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "ED25519 SHA256 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_ED25519_SHA256_HASH_DIGEST);
            return 1;
        }
    }
    else if(type == LIBSSH2_HOSTKEY_TYPE_ECDSA_256) {

#if LIBSSH2_MD5
        md5_hash = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_MD5);
        if(!md5_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_MD5)");
            return 1;
        }

        calculate_digest(md5_hash, MD5_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_ECDSA_MD5_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "ECDSA MD5 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_ECDSA_MD5_HASH_DIGEST);
            return 1;
        }
#endif

        sha1_hash = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_SHA1);
        if(!sha1_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_SHA1)");
            return 1;
        }

        calculate_digest(sha1_hash, SHA1_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_ECDSA_SHA1_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "ECDSA SHA1 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_ECDSA_SHA1_HASH_DIGEST);
            return 1;
        }

        sha256_hash = libssh2_hostkey_hash(session,
                                           LIBSSH2_HOSTKEY_HASH_SHA256);
        if(!sha256_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_SHA256)");
            return 1;
        }

        calculate_digest(sha256_hash, SHA256_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_ECDSA_SHA256_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "ECDSA SHA256 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_ECDSA_SHA256_HASH_DIGEST);
            return 1;
        }
    }
    else if(type == LIBSSH2_HOSTKEY_TYPE_RSA) {

#if LIBSSH2_MD5
        md5_hash = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_MD5);
        if(!md5_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_MD5)");
            return 1;
        }

        calculate_digest(md5_hash, MD5_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_RSA_MD5_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "MD5 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_RSA_MD5_HASH_DIGEST);
            return 1;
        }
#endif

        sha1_hash = libssh2_hostkey_hash(session, LIBSSH2_HOSTKEY_HASH_SHA1);
        if(!sha1_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_SHA1)");
            return 1;
        }

        calculate_digest(sha1_hash, SHA1_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_RSA_SHA1_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "SHA1 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_RSA_SHA1_HASH_DIGEST);
            return 1;
        }

        sha256_hash = libssh2_hostkey_hash(session,
                                           LIBSSH2_HOSTKEY_HASH_SHA256);
        if(!sha256_hash) {
            print_last_session_error(
                "libssh2_hostkey_hash(LIBSSH2_HOSTKEY_HASH_SHA256)");
            return 1;
        }

        calculate_digest(sha256_hash, SHA256_HASH_SIZE, buf, BUFSIZ);

        if(strcmp(buf, EXPECTED_RSA_SHA256_HASH_DIGEST) != 0) {
            fprintf(stderr,
                    "SHA256 hash not as expected - digest %s != %s\n",
                    buf, EXPECTED_RSA_SHA256_HASH_DIGEST);
            return 1;
        }
    }
    else {
        fprintf(stderr, "Unexpected type of hostkey: %i\n", type);
        return 1;
    }

    return 0;
}
