/* Copyright (C) Sara Golemon <sarag@libssh2.org>
 * Copyright (C) Daniel Stenberg
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms,
 * with or without modification, are permitted provided
 * that the following conditions are met:
 *
 *   Redistributions of source code must retain the above
 *   copyright notice, this list of conditions and the
 *   following disclaimer.
 *
 *   Redistributions in binary form must reproduce the above
 *   copyright notice, this list of conditions and the following
 *   disclaimer in the documentation and/or other materials
 *   provided with the distribution.
 *
 *   Neither the name of the copyright holder nor the names
 *   of any other contributors may be used to endorse or
 *   promote products derived from this software without
 *   specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 * CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
 * USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
 * OF SUCH DAMAGE.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "libssh2_priv.h"

/* Needed for struct iovec on some platforms */
#ifdef HAVE_SYS_UIO_H
#include <sys/uio.h>
#endif

#if LIBSSH2_RSA
/* ***********
 * ssh-rsa *
 *********** */

static int hostkey_method_ssh_rsa_dtor(LIBSSH2_SESSION * session,
                                       void **abstract);

/*
 * hostkey_method_ssh_rsa_init
 *
 * Initialize the server hostkey working area with e/n pair
 */
static int
hostkey_method_ssh_rsa_init(LIBSSH2_SESSION * session,
                            const unsigned char *hostkey_data,
                            size_t hostkey_data_len,
                            void **abstract)
{
    libssh2_rsa_ctx *rsactx;
    unsigned char *e, *n, *type;
    size_t e_len, n_len, type_len;
    struct string_buf buf;

    if(*abstract) {
        hostkey_method_ssh_rsa_dtor(session, abstract);
        *abstract = NULL;
    }

    if(hostkey_data_len < 19) {
        _libssh2_debug((session, LIBSSH2_TRACE_ERROR,
                       "host key length too short"));
        return -1;
    }

    buf.data = (unsigned char *)hostkey_data;
    buf.dataptr = buf.data;
    buf.len = hostkey_data_len;

    if(_libssh2_get_string(&buf, &type, &type_len)) {
        return -1;
    }

    /* we accept one of 3 header types */
#if LIBSSH2_RSA_SHA1
    if(type_len == 7 && strncmp("ssh-rsa", (char *)type, 7) == 0) {
        /* ssh-rsa */
    }
    else
#endif
#if LIBSSH2_RSA_SHA2
    if(type_len == 12 && strncmp("rsa-sha2-256", (char *)type, 12) == 0) {
        /* rsa-sha2-256 */
    }
    else if(type_len == 12 && strncmp("rsa-sha2-512", (char *)type, 12) == 0) {
        /* rsa-sha2-512 */
    }
    else
#endif
    {
        _libssh2_debug((session, LIBSSH2_TRACE_ERROR,
                       "unexpected rsa type: %.*s", (int)type_len, type));
        return -1;
    }

    if(_libssh2_get_string(&buf, &e, &e_len))
        return -1;

    if(_libssh2_get_string(&buf, &n, &n_len))
        return -1;

    if(!_libssh2_eob(&buf))
        return -1;

    if(_libssh2_rsa_new(&rsactx,
                        e, (unsigned long)e_len,
                        n, (unsigned long)n_len,
                        NULL, 0, NULL, 0, NULL, 0,
                        NULL, 0, NULL, 0, NULL, 0)) {
        return -1;
    }

    *abstract = rsactx;

    return 0;
}

/*
 * hostkey_method_ssh_rsa_initPEM
 *
 * Load a Private Key from a PEM file
 */
static int
hostkey_method_ssh_rsa_initPEM(LIBSSH2_SESSION * session,
                               const char *privkeyfile,
                               unsigned const char *passphrase,
                               void **abstract)
{
    libssh2_rsa_ctx *rsactx;
    int ret;

    if(*abstract) {
        hostkey_method_ssh_rsa_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_rsa_new_private(&rsactx, session, privkeyfile, passphrase);
    if(ret) {
        return -1;
    }

    *abstract = rsactx;

    return 0;
}

/*
 * hostkey_method_ssh_rsa_initPEMFromMemory
 *
 * Load a Private Key from a memory
 */
static int
hostkey_method_ssh_rsa_initPEMFromMemory(LIBSSH2_SESSION * session,
                                         const char *privkeyfiledata,
                                         size_t privkeyfiledata_len,
                                         unsigned const char *passphrase,
                                         void **abstract)
{
    libssh2_rsa_ctx *rsactx;
    int ret;

    if(*abstract) {
        hostkey_method_ssh_rsa_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_rsa_new_private_frommemory(&rsactx, session,
                                              privkeyfiledata,
                                              privkeyfiledata_len, passphrase);
    if(ret) {
        return -1;
    }

    *abstract = rsactx;

    return 0;
}

#if LIBSSH2_RSA_SHA1
/*
 * hostkey_method_ssh_rsa_sign
 *
 * Verify signature created by remote
 */
static int
hostkey_method_ssh_rsa_sig_verify(LIBSSH2_SESSION * session,
                                  const unsigned char *sig,
                                  size_t sig_len,
                                  const unsigned char *m,
                                  size_t m_len, void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);
    (void)session;

    /* Skip past keyname_len(4) + keyname(7){"ssh-rsa"} + signature_len(4) */
    if(sig_len < 15)
        return -1;

    sig += 15;
    sig_len -= 15;
    return _libssh2_rsa_sha1_verify(rsactx, sig, sig_len, m, m_len);
}

/*
 * hostkey_method_ssh_rsa_signv
 *
 * Construct a signature from an array of vectors
 */
static int
hostkey_method_ssh_rsa_signv(LIBSSH2_SESSION * session,
                             unsigned char **signature,
                             size_t *signature_len,
                             int veccount,
                             const struct iovec datavec[],
                             void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);

#ifdef _libssh2_rsa_sha1_signv
    return _libssh2_rsa_sha1_signv(session, signature, signature_len,
                                   veccount, datavec, rsactx);
#else
    int ret;
    int i;
    unsigned char hash[SHA_DIGEST_LENGTH];
    libssh2_sha1_ctx ctx;

    if(!libssh2_sha1_init(&ctx)) {
        return -1;
    }
    for(i = 0; i < veccount; i++) {
        if(!libssh2_sha1_update(ctx,
                                datavec[i].iov_base, datavec[i].iov_len)) {
            return -1;
        }
    }
    if(!libssh2_sha1_final(ctx, hash)) {
        return -1;
    }

    ret = _libssh2_rsa_sha1_sign(session, rsactx, hash, SHA_DIGEST_LENGTH,
                                 signature, signature_len);
    if(ret) {
        return -1;
    }

    return 0;
#endif
}
#endif

/*
 * hostkey_method_ssh_rsa_sha2_256_sig_verify
 *
 * Verify signature created by remote
 */
#if LIBSSH2_RSA_SHA2

static int
hostkey_method_ssh_rsa_sha2_256_sig_verify(LIBSSH2_SESSION * session,
                                           const unsigned char *sig,
                                           size_t sig_len,
                                           const unsigned char *m,
                                           size_t m_len, void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);
    (void)session;

    /* Skip past keyname_len(4) + keyname(12){"rsa-sha2-256"} +
       signature_len(4) */
    if(sig_len < 20)
        return -1;

    sig += 20;
    sig_len -= 20;
    return _libssh2_rsa_sha2_verify(rsactx, SHA256_DIGEST_LENGTH, sig, sig_len,
                                    m, m_len);
}

/*
 * hostkey_method_ssh_rsa_sha2_256_signv
 *
 * Construct a signature from an array of vectors
 */

static int
hostkey_method_ssh_rsa_sha2_256_signv(LIBSSH2_SESSION * session,
                                      unsigned char **signature,
                                      size_t *signature_len,
                                      int veccount,
                                      const struct iovec datavec[],
                                      void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);

#ifdef _libssh2_rsa_sha2_256_signv
    return _libssh2_rsa_sha2_256_signv(session, signature, signature_len,
                                       veccount, datavec, rsactx);
#else
    int ret;
    int i;
    unsigned char hash[SHA256_DIGEST_LENGTH];
    libssh2_sha256_ctx ctx;

    if(!libssh2_sha256_init(&ctx)) {
        return -1;
    }
    for(i = 0; i < veccount; i++) {
        if(!libssh2_sha256_update(ctx,
                                  datavec[i].iov_base, datavec[i].iov_len)) {
            return -1;
        }
    }
    if(!libssh2_sha256_final(ctx, hash)) {
        return -1;
    }

    ret = _libssh2_rsa_sha2_sign(session, rsactx, hash, SHA256_DIGEST_LENGTH,
                                 signature, signature_len);
    if(ret) {
        return -1;
    }

    return 0;
#endif
}

/*
 * hostkey_method_ssh_rsa_sha2_512_sig_verify
 *
 * Verify signature created by remote
 */

static int
hostkey_method_ssh_rsa_sha2_512_sig_verify(LIBSSH2_SESSION * session,
                                           const unsigned char *sig,
                                           size_t sig_len,
                                           const unsigned char *m,
                                           size_t m_len, void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);
    (void)session;

    /* Skip past keyname_len(4) + keyname(12){"rsa-sha2-512"} +
       signature_len(4) */
    if(sig_len < 20)
        return -1;

    sig += 20;
    sig_len -= 20;
    return _libssh2_rsa_sha2_verify(rsactx, SHA512_DIGEST_LENGTH, sig,
                                    sig_len, m, m_len);
}


/*
 * hostkey_method_ssh_rsa_sha2_512_signv
 *
 * Construct a signature from an array of vectors
 */
static int
hostkey_method_ssh_rsa_sha2_512_signv(LIBSSH2_SESSION * session,
                                      unsigned char **signature,
                                      size_t *signature_len,
                                      int veccount,
                                      const struct iovec datavec[],
                                      void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);

#ifdef _libssh2_rsa_sha2_512_signv
    return _libssh2_rsa_sha2_512_signv(session, signature, signature_len,
                                       veccount, datavec, rsactx);
#else
    int ret;
    int i;
    unsigned char hash[SHA512_DIGEST_LENGTH];
    libssh2_sha512_ctx ctx;

    if(!libssh2_sha512_init(&ctx)) {
        return -1;
    }
    for(i = 0; i < veccount; i++) {
        if(!libssh2_sha512_update(ctx,
                                  datavec[i].iov_base, datavec[i].iov_len)) {
            return -1;
        }
    }
    if(!libssh2_sha512_final(ctx, hash)) {
        return -1;
    }

    ret = _libssh2_rsa_sha2_sign(session, rsactx, hash, SHA512_DIGEST_LENGTH,
                                 signature, signature_len);
    if(ret) {
        return -1;
    }

    return 0;
#endif
}

#endif /* LIBSSH2_RSA_SHA2 */


/*
 * hostkey_method_ssh_rsa_dtor
 *
 * Shutdown the hostkey
 */
static int
hostkey_method_ssh_rsa_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    libssh2_rsa_ctx *rsactx = (libssh2_rsa_ctx *) (*abstract);
    (void)session;

    _libssh2_rsa_free(rsactx);

    *abstract = NULL;

    return 0;
}

#if LIBSSH2_RSA_SHA1

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa = {
    "ssh-rsa",
    SHA_DIGEST_LENGTH,
    hostkey_method_ssh_rsa_init,
    hostkey_method_ssh_rsa_initPEM,
    hostkey_method_ssh_rsa_initPEMFromMemory,
    hostkey_method_ssh_rsa_sig_verify,
    hostkey_method_ssh_rsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_rsa_dtor,
};

#endif /* LIBSSH2_RSA_SHA1 */

#if LIBSSH2_RSA_SHA2

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa_sha2_256 = {
    "rsa-sha2-256",
    SHA256_DIGEST_LENGTH,
    hostkey_method_ssh_rsa_init,
    hostkey_method_ssh_rsa_initPEM,
    hostkey_method_ssh_rsa_initPEMFromMemory,
    hostkey_method_ssh_rsa_sha2_256_sig_verify,
    hostkey_method_ssh_rsa_sha2_256_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_rsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa_sha2_512 = {
    "rsa-sha2-512",
    SHA512_DIGEST_LENGTH,
    hostkey_method_ssh_rsa_init,
    hostkey_method_ssh_rsa_initPEM,
    hostkey_method_ssh_rsa_initPEMFromMemory,
    hostkey_method_ssh_rsa_sha2_512_sig_verify,
    hostkey_method_ssh_rsa_sha2_512_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_rsa_dtor,
};

#endif /* LIBSSH2_RSA_SHA2 */

#if LIBSSH2_RSA_SHA1

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa_cert = {
    "ssh-rsa-cert-v01@openssh.com",
    SHA_DIGEST_LENGTH,
    NULL,
    hostkey_method_ssh_rsa_initPEM,
    hostkey_method_ssh_rsa_initPEMFromMemory,
    NULL,
    hostkey_method_ssh_rsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_rsa_dtor,
};

#endif /* LIBSSH2_RSA_SHA1 */

#if LIBSSH2_RSA_SHA2

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa_sha2_256_cert = {
     "rsa-sha2-256-cert-v01@openssh.com",
     SHA256_DIGEST_LENGTH,
     NULL,
     hostkey_method_ssh_rsa_initPEM,
     hostkey_method_ssh_rsa_initPEMFromMemory,
     NULL,
     hostkey_method_ssh_rsa_sha2_256_signv,
     NULL,                       /* encrypt */
     hostkey_method_ssh_rsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_rsa_sha2_512_cert = {
     "rsa-sha2-512-cert-v01@openssh.com",
     SHA512_DIGEST_LENGTH,
     NULL,
     hostkey_method_ssh_rsa_initPEM,
     hostkey_method_ssh_rsa_initPEMFromMemory,
     NULL,
     hostkey_method_ssh_rsa_sha2_512_signv,
     NULL,                       /* encrypt */
     hostkey_method_ssh_rsa_dtor,
};

#endif /* LIBSSH2_RSA_SHA2 */

#endif /* LIBSSH2_RSA */

#if LIBSSH2_DSA
/* ***********
 * ssh-dss *
 *********** */

static int hostkey_method_ssh_dss_dtor(LIBSSH2_SESSION * session,
                                       void **abstract);

/*
 * hostkey_method_ssh_dss_init
 *
 * Initialize the server hostkey working area with p/q/g/y set
 */
static int
hostkey_method_ssh_dss_init(LIBSSH2_SESSION * session,
                            const unsigned char *hostkey_data,
                            size_t hostkey_data_len,
                            void **abstract)
{
    libssh2_dsa_ctx *dsactx;
    unsigned char *p, *q, *g, *y;
    size_t p_len, q_len, g_len, y_len;
    struct string_buf buf;

    if(*abstract) {
        hostkey_method_ssh_dss_dtor(session, abstract);
        *abstract = NULL;
    }

    if(hostkey_data_len < 27) {
        _libssh2_debug((session, LIBSSH2_TRACE_ERROR,
                       "host key length too short"));
        return -1;
    }

    buf.data = (unsigned char *)hostkey_data;
    buf.dataptr = buf.data;
    buf.len = hostkey_data_len;

    if(_libssh2_match_string(&buf, "ssh-dss"))
        return -1;

    if(_libssh2_get_string(&buf, &p, &p_len))
        return -1;

    if(_libssh2_get_string(&buf, &q, &q_len))
        return -1;

    if(_libssh2_get_string(&buf, &g, &g_len))
        return -1;

    if(_libssh2_get_string(&buf, &y, &y_len))
        return -1;

    if(!_libssh2_eob(&buf))
        return -1;

    if(_libssh2_dsa_new(&dsactx,
                        p, (unsigned long)p_len,
                        q, (unsigned long)q_len,
                        g, (unsigned long)g_len,
                        y, (unsigned long)y_len,
                        NULL, 0)) {
        return -1;
    }

    *abstract = dsactx;

    return 0;
}

/*
 * hostkey_method_ssh_dss_initPEM
 *
 * Load a Private Key from a PEM file
 */
static int
hostkey_method_ssh_dss_initPEM(LIBSSH2_SESSION * session,
                               const char *privkeyfile,
                               unsigned const char *passphrase,
                               void **abstract)
{
    libssh2_dsa_ctx *dsactx;
    int ret;

    if(*abstract) {
        hostkey_method_ssh_dss_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_dsa_new_private(&dsactx, session, privkeyfile, passphrase);
    if(ret) {
        return -1;
    }

    *abstract = dsactx;

    return 0;
}

/*
 * hostkey_method_ssh_dss_initPEMFromMemory
 *
 * Load a Private Key from memory
 */
static int
hostkey_method_ssh_dss_initPEMFromMemory(LIBSSH2_SESSION * session,
                                         const char *privkeyfiledata,
                                         size_t privkeyfiledata_len,
                                         unsigned const char *passphrase,
                                         void **abstract)
{
    libssh2_dsa_ctx *dsactx;
    int ret;

    if(*abstract) {
        hostkey_method_ssh_dss_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_dsa_new_private_frommemory(&dsactx, session,
                                              privkeyfiledata,
                                              privkeyfiledata_len, passphrase);
    if(ret) {
        return -1;
    }

    *abstract = dsactx;

    return 0;
}

/*
 * libssh2_hostkey_method_ssh_dss_sign
 *
 * Verify signature created by remote
 */
static int
hostkey_method_ssh_dss_sig_verify(LIBSSH2_SESSION * session,
                                  const unsigned char *sig,
                                  size_t sig_len,
                                  const unsigned char *m,
                                  size_t m_len, void **abstract)
{
    libssh2_dsa_ctx *dsactx = (libssh2_dsa_ctx *) (*abstract);

    /* Skip past keyname_len(4) + keyname(7){"ssh-dss"} + signature_len(4) */
    if(sig_len != 55) {
        return _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                              "Invalid DSS signature length");
    }

    sig += 15;
    sig_len -= 15;

    return _libssh2_dsa_sha1_verify(dsactx, sig, m, m_len);
}

/*
 * hostkey_method_ssh_dss_signv
 *
 * Construct a signature from an array of vectors
 */
static int
hostkey_method_ssh_dss_signv(LIBSSH2_SESSION * session,
                             unsigned char **signature,
                             size_t *signature_len,
                             int veccount,
                             const struct iovec datavec[],
                             void **abstract)
{
    libssh2_dsa_ctx *dsactx = (libssh2_dsa_ctx *) (*abstract);
    unsigned char hash[SHA_DIGEST_LENGTH];
    libssh2_sha1_ctx ctx;
    int i;

    if(!libssh2_sha1_init(&ctx)) {
        *signature = NULL;
        *signature_len = 0;
        return -1;
    }

    *signature = LIBSSH2_CALLOC(session, 2 * SHA_DIGEST_LENGTH);
    if(!*signature) {
        return -1;
    }

    *signature_len = 2 * SHA_DIGEST_LENGTH;

    for(i = 0; i < veccount; i++) {
        if(!libssh2_sha1_update(ctx,
                                datavec[i].iov_base, datavec[i].iov_len)) {
            return -1;
        }
    }
    if(!libssh2_sha1_final(ctx, hash)) {
        return -1;
    }

    if(_libssh2_dsa_sha1_sign(dsactx, hash, SHA_DIGEST_LENGTH, *signature)) {
        LIBSSH2_FREE(session, *signature);
        return -1;
    }

    return 0;
}

/*
 * libssh2_hostkey_method_ssh_dss_dtor
 *
 * Shutdown the hostkey method
 */
static int
hostkey_method_ssh_dss_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    libssh2_dsa_ctx *dsactx = (libssh2_dsa_ctx *) (*abstract);
    (void)session;

    _libssh2_dsa_free(dsactx);

    *abstract = NULL;

    return 0;
}

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_dss = {
    "ssh-dss",
    SHA_DIGEST_LENGTH,
    hostkey_method_ssh_dss_init,
    hostkey_method_ssh_dss_initPEM,
    hostkey_method_ssh_dss_initPEMFromMemory,
    hostkey_method_ssh_dss_sig_verify,
    hostkey_method_ssh_dss_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_dss_dtor,
};
#endif /* LIBSSH2_DSA */

#if LIBSSH2_ECDSA

/* ***********
 * ecdsa-sha2-nistp256/384/521 *
 *********** */

static int
hostkey_method_ssh_ecdsa_dtor(LIBSSH2_SESSION * session,
                              void **abstract);

/*
 * hostkey_method_ssh_ecdsa_init
 *
 * Initialize the server hostkey working area with e/n pair
 */
static int
hostkey_method_ssh_ecdsa_init(LIBSSH2_SESSION * session,
                              const unsigned char *hostkey_data,
                              size_t hostkey_data_len,
                              void **abstract)
{
    libssh2_ecdsa_ctx *ecdsactx = NULL;
    unsigned char *type_str, *domain, *public_key;
    size_t key_len, len;
    libssh2_curve_type type;
    struct string_buf buf;

    if(abstract && *abstract) {
        hostkey_method_ssh_ecdsa_dtor(session, abstract);
        *abstract = NULL;
    }

    if(hostkey_data_len < 39) {
        _libssh2_debug((session, LIBSSH2_TRACE_ERROR,
                       "host key length too short"));
        return -1;
    }

    buf.data = (unsigned char *)hostkey_data;
    buf.dataptr = buf.data;
    buf.len = hostkey_data_len;

    if(_libssh2_get_string(&buf, &type_str, &len) || len != 19)
        return -1;

    if(strncmp((char *) type_str, "ecdsa-sha2-nistp256", 19) == 0) {
        type = LIBSSH2_EC_CURVE_NISTP256;
    }
    else if(strncmp((char *) type_str, "ecdsa-sha2-nistp384", 19) == 0) {
        type = LIBSSH2_EC_CURVE_NISTP384;
    }
    else if(strncmp((char *) type_str, "ecdsa-sha2-nistp521", 19) == 0) {
        type = LIBSSH2_EC_CURVE_NISTP521;
    }
    else {
        return -1;
    }

    if(_libssh2_get_string(&buf, &domain, &len) || len != 8)
        return -1;

    if(type == LIBSSH2_EC_CURVE_NISTP256 &&
       strncmp((char *)domain, "nistp256", 8) != 0) {
        return -1;
    }
    else if(type == LIBSSH2_EC_CURVE_NISTP384 &&
            strncmp((char *)domain, "nistp384", 8) != 0) {
        return -1;
    }
    else if(type == LIBSSH2_EC_CURVE_NISTP521 &&
            strncmp((char *)domain, "nistp521", 8) != 0) {
        return -1;
    }

    /* public key */
    if(_libssh2_get_string(&buf, &public_key, &key_len))
        return -1;

    if(!_libssh2_eob(&buf))
        return -1;

    if(_libssh2_ecdsa_curve_name_with_octal_new(&ecdsactx, public_key,
                                                key_len, type))
        return -1;

    if(abstract)
        *abstract = ecdsactx;

    return 0;
}

/*
 * hostkey_method_ssh_ecdsa_initPEM
 *
 * Load a Private Key from a PEM file
 */
static int
hostkey_method_ssh_ecdsa_initPEM(LIBSSH2_SESSION * session,
                                 const char *privkeyfile,
                                 unsigned const char *passphrase,
                                 void **abstract)
{
    libssh2_ecdsa_ctx *ec_ctx = NULL;
    int ret;

    if(abstract && *abstract) {
        hostkey_method_ssh_ecdsa_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_ecdsa_new_private(&ec_ctx, session,
                                     privkeyfile, passphrase);

    if(abstract)
        *abstract = ec_ctx;

    return ret;
}

/*
 * hostkey_method_ssh_ecdsa_initPEMFromMemory
 *
 * Load a Private Key from memory
 */
static int
hostkey_method_ssh_ecdsa_initPEMFromMemory(LIBSSH2_SESSION * session,
                                           const char *privkeyfiledata,
                                           size_t privkeyfiledata_len,
                                           unsigned const char *passphrase,
                                           void **abstract)
{
    libssh2_ecdsa_ctx *ec_ctx = NULL;
    int ret;

    if(abstract && *abstract) {
        hostkey_method_ssh_ecdsa_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_ecdsa_new_private_frommemory(&ec_ctx, session,
                                                privkeyfiledata,
                                                privkeyfiledata_len,
                                                passphrase);
    if(ret) {
        return -1;
    }

    if(abstract)
        *abstract = ec_ctx;

    return 0;
}

/*
 * hostkey_method_ecdsa_sig_verify
 *
 * Verify signature created by remote
 */
static int
hostkey_method_ssh_ecdsa_sig_verify(LIBSSH2_SESSION * session,
                                    const unsigned char *sig,
                                    size_t sig_len,
                                    const unsigned char *m,
                                    size_t m_len, void **abstract)
{
    unsigned char *r, *s, *name;
    size_t r_len, s_len, name_len;
    uint32_t len;
    struct string_buf buf;
    libssh2_ecdsa_ctx *ctx = (libssh2_ecdsa_ctx *) (*abstract);

    (void)session;

    if(sig_len < 35)
        return -1;

    /* keyname_len(4) + keyname(19){"ecdsa-sha2-nistp256"} +
       signature_len(4) */
    buf.data = (unsigned char *)sig;
    buf.dataptr = buf.data;
    buf.len = sig_len;

    if(_libssh2_get_string(&buf, &name, &name_len) || name_len != 19)
        return -1;

    if(_libssh2_get_u32(&buf, &len) != 0 || len < 8)
        return -1;

    if(_libssh2_get_string(&buf, &r, &r_len))
        return -1;

    if(_libssh2_get_string(&buf, &s, &s_len))
        return -1;

    return _libssh2_ecdsa_verify(ctx, r, r_len, s, s_len, m, m_len);
}


#define LIBSSH2_HOSTKEY_METHOD_EC_SIGNV_HASH(digest_type)                \
    do {                                                                 \
        unsigned char hash[SHA##digest_type##_DIGEST_LENGTH];            \
        libssh2_sha##digest_type##_ctx ctx;                              \
        int i;                                                           \
        if(!libssh2_sha##digest_type##_init(&ctx)) {                     \
            ret = -1;                                                    \
            break;                                                       \
        }                                                                \
        for(i = 0; i < veccount; i++) {                                  \
            if(!libssh2_sha##digest_type##_update(ctx,                   \
                                                  datavec[i].iov_base,   \
                                                  datavec[i].iov_len)) { \
                ret = -1;                                                \
                break;                                                   \
            }                                                            \
        }                                                                \
        if(ret == -1) {                                                  \
            break;                                                       \
        }                                                                \
        if(!libssh2_sha##digest_type##_final(ctx, hash)) {               \
            ret = -1;                                                    \
            break;                                                       \
        }                                                                \
        ret = _libssh2_ecdsa_sign(session, ec_ctx, hash,                 \
                                  SHA##digest_type##_DIGEST_LENGTH,      \
                                  signature, signature_len);             \
    } while(0)


/*
 * hostkey_method_ecdsa_signv
 *
 * Construct a signature from an array of vectors
 */
static int
hostkey_method_ssh_ecdsa_signv(LIBSSH2_SESSION * session,
                               unsigned char **signature,
                               size_t *signature_len,
                               int veccount,
                               const struct iovec datavec[],
                               void **abstract)
{
    libssh2_ecdsa_ctx *ec_ctx = (libssh2_ecdsa_ctx *) (*abstract);
    libssh2_curve_type type = _libssh2_ecdsa_get_curve_type(ec_ctx);
    int ret = 0;

    if(type == LIBSSH2_EC_CURVE_NISTP256) {
        LIBSSH2_HOSTKEY_METHOD_EC_SIGNV_HASH(256);
    }
    else if(type == LIBSSH2_EC_CURVE_NISTP384) {
        LIBSSH2_HOSTKEY_METHOD_EC_SIGNV_HASH(384);
    }
    else if(type == LIBSSH2_EC_CURVE_NISTP521) {
        LIBSSH2_HOSTKEY_METHOD_EC_SIGNV_HASH(512);
    }
    else {
        return -1;
    }

    return ret;
}

/*
 * hostkey_method_ssh_ecdsa_dtor
 *
 * Shutdown the hostkey by freeing EC_KEY context
 */
static int
hostkey_method_ssh_ecdsa_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    libssh2_ecdsa_ctx *keyctx = (libssh2_ecdsa_ctx *) (*abstract);
    (void)session;

    if(keyctx)
        _libssh2_ecdsa_free(keyctx);

    *abstract = NULL;

    return 0;
}

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp256 = {
    "ecdsa-sha2-nistp256",
    SHA256_DIGEST_LENGTH,
    hostkey_method_ssh_ecdsa_init,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    hostkey_method_ssh_ecdsa_sig_verify,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp384 = {
    "ecdsa-sha2-nistp384",
    SHA384_DIGEST_LENGTH,
    hostkey_method_ssh_ecdsa_init,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    hostkey_method_ssh_ecdsa_sig_verify,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp521 = {
    "ecdsa-sha2-nistp521",
    SHA512_DIGEST_LENGTH,
    hostkey_method_ssh_ecdsa_init,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    hostkey_method_ssh_ecdsa_sig_verify,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp256_cert = {
    "ecdsa-sha2-nistp256-cert-v01@openssh.com",
    SHA256_DIGEST_LENGTH,
    NULL,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    NULL,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp384_cert = {
    "ecdsa-sha2-nistp384-cert-v01@openssh.com",
    SHA384_DIGEST_LENGTH,
    NULL,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    NULL,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ecdsa_ssh_nistp521_cert = {
    "ecdsa-sha2-nistp521-cert-v01@openssh.com",
    SHA512_DIGEST_LENGTH,
    NULL,
    hostkey_method_ssh_ecdsa_initPEM,
    hostkey_method_ssh_ecdsa_initPEMFromMemory,
    NULL,
    hostkey_method_ssh_ecdsa_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ecdsa_dtor,
};

#endif /* LIBSSH2_ECDSA */

#if LIBSSH2_ED25519

/* ***********
 * ed25519 *
 *********** */

static int hostkey_method_ssh_ed25519_dtor(LIBSSH2_SESSION * session,
                                           void **abstract);

/*
 * hostkey_method_ssh_ed25519_init
 *
 * Initialize the server hostkey working area with e/n pair
 */
static int
hostkey_method_ssh_ed25519_init(LIBSSH2_SESSION * session,
                                const unsigned char *hostkey_data,
                                size_t hostkey_data_len,
                                void **abstract)
{
    size_t key_len;
    unsigned char *key;
    libssh2_ed25519_ctx *ctx = NULL;
    struct string_buf buf;

    if(*abstract) {
        hostkey_method_ssh_ed25519_dtor(session, abstract);
        *abstract = NULL;
    }

    if(hostkey_data_len < 19) {
        _libssh2_debug((session, LIBSSH2_TRACE_ERROR,
                       "host key length too short"));
        return -1;
    }

    buf.data = (unsigned char *)hostkey_data;
    buf.dataptr = buf.data;
    buf.len = hostkey_data_len;

    if(_libssh2_match_string(&buf, "ssh-ed25519"))
        return -1;

    /* public key */
    if(_libssh2_get_string(&buf, &key, &key_len))
        return -1;

    if(!_libssh2_eob(&buf))
        return -1;

    if(_libssh2_ed25519_new_public(&ctx, session, key, key_len) != 0) {
        return -1;
    }

    *abstract = ctx;

    return 0;
}

/*
 * hostkey_method_ssh_ed25519_initPEM
 *
 * Load a Private Key from a PEM file
 */
static int
hostkey_method_ssh_ed25519_initPEM(LIBSSH2_SESSION * session,
                                   const char *privkeyfile,
                                   unsigned const char *passphrase,
                                   void **abstract)
{
    libssh2_ed25519_ctx *ec_ctx = NULL;
    int ret;

    if(*abstract) {
        hostkey_method_ssh_ed25519_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_ed25519_new_private(&ec_ctx, session,
                                       privkeyfile, passphrase);
    if(ret) {
        return -1;
    }

    *abstract = ec_ctx;

    return ret;
}

/*
 * hostkey_method_ssh_ed25519_initPEMFromMemory
 *
 * Load a Private Key from memory
 */
static int
hostkey_method_ssh_ed25519_initPEMFromMemory(LIBSSH2_SESSION * session,
                                             const char *privkeyfiledata,
                                             size_t privkeyfiledata_len,
                                             unsigned const char *passphrase,
                                             void **abstract)
{
    libssh2_ed25519_ctx *ed_ctx = NULL;
    int ret;

    if(abstract && *abstract) {
        hostkey_method_ssh_ed25519_dtor(session, abstract);
        *abstract = NULL;
    }

    ret = _libssh2_ed25519_new_private_frommemory(&ed_ctx, session,
                                                  privkeyfiledata,
                                                  privkeyfiledata_len,
                                                  passphrase);
    if(ret) {
        return -1;
    }

    if(abstract)
        *abstract = ed_ctx;

    return 0;
}

/*
 * hostkey_method_ssh_ed25519_sig_verify
 *
 * Verify signature created by remote
 */
static int
hostkey_method_ssh_ed25519_sig_verify(LIBSSH2_SESSION * session,
                                      const unsigned char *sig,
                                      size_t sig_len,
                                      const unsigned char *m,
                                      size_t m_len, void **abstract)
{
    libssh2_ed25519_ctx *ctx = (libssh2_ed25519_ctx *) (*abstract);
    (void)session;

    if(sig_len < 19)
        return -1;

    /* Skip past keyname_len(4) + keyname(11){"ssh-ed25519"} +
       signature_len(4) */
    sig += 19;
    sig_len -= 19;

    if(sig_len != LIBSSH2_ED25519_SIG_LEN)
        return -1;

    return _libssh2_ed25519_verify(ctx, sig, sig_len, m, m_len);
}

/*
 * hostkey_method_ssh_ed25519_signv
 *
 * Construct a signature from an array of vectors
 */
static int
hostkey_method_ssh_ed25519_signv(LIBSSH2_SESSION * session,
                                 unsigned char **signature,
                                 size_t *signature_len,
                                 int veccount,
                                 const struct iovec datavec[],
                                 void **abstract)
{
    libssh2_ed25519_ctx *ctx = (libssh2_ed25519_ctx *) (*abstract);

    if(veccount != 1) {
        return -1;
    }

    return _libssh2_ed25519_sign(ctx, session, signature, signature_len,
                                 (const uint8_t *)datavec[0].iov_base,
                                 datavec[0].iov_len);
}


/*
 * hostkey_method_ssh_ed25519_dtor
 *
 * Shutdown the hostkey by freeing key context
 */
static int
hostkey_method_ssh_ed25519_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    libssh2_ed25519_ctx *keyctx = (libssh2_ed25519_ctx*) (*abstract);
    (void)session;

    if(keyctx)
        _libssh2_ed25519_free(keyctx);

    *abstract = NULL;

    return 0;
}

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_ed25519 = {
    "ssh-ed25519",
    SHA256_DIGEST_LENGTH,
    hostkey_method_ssh_ed25519_init,
    hostkey_method_ssh_ed25519_initPEM,
    hostkey_method_ssh_ed25519_initPEMFromMemory,
    hostkey_method_ssh_ed25519_sig_verify,
    hostkey_method_ssh_ed25519_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ed25519_dtor,
};

static const LIBSSH2_HOSTKEY_METHOD hostkey_method_ssh_ed25519_cert = {
    "ssh-ed25519-cert-v01@openssh.com",
    SHA256_DIGEST_LENGTH,
    hostkey_method_ssh_ed25519_init,
    hostkey_method_ssh_ed25519_initPEM,
    hostkey_method_ssh_ed25519_initPEMFromMemory,
    hostkey_method_ssh_ed25519_sig_verify,
    hostkey_method_ssh_ed25519_signv,
    NULL,                       /* encrypt */
    hostkey_method_ssh_ed25519_dtor,
};

#endif /* LIBSSH2_ED25519 */


static const LIBSSH2_HOSTKEY_METHOD *hostkey_methods[] = {
#if LIBSSH2_ECDSA
    &hostkey_method_ecdsa_ssh_nistp256,
    &hostkey_method_ecdsa_ssh_nistp384,
    &hostkey_method_ecdsa_ssh_nistp521,
    &hostkey_method_ecdsa_ssh_nistp256_cert,
    &hostkey_method_ecdsa_ssh_nistp384_cert,
    &hostkey_method_ecdsa_ssh_nistp521_cert,
#endif
#if LIBSSH2_ED25519
    &hostkey_method_ssh_ed25519,
    &hostkey_method_ssh_ed25519_cert,
#endif
#if LIBSSH2_RSA
#if LIBSSH2_RSA_SHA2
    &hostkey_method_ssh_rsa_sha2_512,
    &hostkey_method_ssh_rsa_sha2_256,
    &hostkey_method_ssh_rsa_sha2_512_cert,
    &hostkey_method_ssh_rsa_sha2_256_cert,
#endif /* LIBSSH2_RSA_SHA2 */
#if LIBSSH2_RSA_SHA1
    &hostkey_method_ssh_rsa,
    &hostkey_method_ssh_rsa_cert,
#endif /* LIBSSH2_RSA_SHA1 */
#endif /* LIBSSH2_RSA */
#if LIBSSH2_DSA
    &hostkey_method_ssh_dss,
#endif /* LIBSSH2_DSA */
    NULL
};

const LIBSSH2_HOSTKEY_METHOD **
libssh2_hostkey_methods(void)
{
    return hostkey_methods;
}

/*
 * libssh2_hostkey_hash
 *
 * Returns hash signature
 * Returned buffer should NOT be freed
 * Length of buffer is determined by hash type
 * i.e. MD5 == 16, SHA1 == 20, SHA256 == 32
 */
LIBSSH2_API const char *
libssh2_hostkey_hash(LIBSSH2_SESSION * session, int hash_type)
{
    switch(hash_type) {
#if LIBSSH2_MD5
    case LIBSSH2_HOSTKEY_HASH_MD5:
        return (session->server_hostkey_md5_valid)
          ? (char *) session->server_hostkey_md5
          : NULL;
#endif /* LIBSSH2_MD5 */
    case LIBSSH2_HOSTKEY_HASH_SHA1:
        return (session->server_hostkey_sha1_valid)
          ? (char *) session->server_hostkey_sha1
          : NULL;
    case LIBSSH2_HOSTKEY_HASH_SHA256:
        return (session->server_hostkey_sha256_valid)
          ? (char *) session->server_hostkey_sha256
          : NULL;
    default:
        return NULL;
    }
}

static int hostkey_type(const unsigned char *hostkey, size_t len)
{
    static const unsigned char rsa[] = {
        0, 0, 0, 0x07, 's', 's', 'h', '-', 'r', 's', 'a'
    };
#if LIBSSH2_DSA
    static const unsigned char dss[] = {
        0, 0, 0, 0x07, 's', 's', 'h', '-', 'd', 's', 's'
    };
#endif
    static const unsigned char ecdsa_256[] = {
        0, 0, 0, 0x13, 'e', 'c', 'd', 's', 'a', '-', 's', 'h', 'a', '2', '-',
        'n', 'i', 's', 't', 'p', '2', '5', '6'
    };
    static const unsigned char ecdsa_384[] = {
        0, 0, 0, 0x13, 'e', 'c', 'd', 's', 'a', '-', 's', 'h', 'a', '2', '-',
        'n', 'i', 's', 't', 'p', '3', '8', '4'
    };
    static const unsigned char ecdsa_521[] = {
        0, 0, 0, 0x13, 'e', 'c', 'd', 's', 'a', '-', 's', 'h', 'a', '2', '-',
        'n', 'i', 's', 't', 'p', '5', '2', '1'
    };
    static const unsigned char ed25519[] = {
        0, 0, 0, 0x0b, 's', 's', 'h', '-', 'e', 'd', '2', '5', '5', '1', '9'
    };

    if(len < 11)
        return LIBSSH2_HOSTKEY_TYPE_UNKNOWN;

    if(!memcmp(rsa, hostkey, 11))
        return LIBSSH2_HOSTKEY_TYPE_RSA;

#if LIBSSH2_DSA
    if(!memcmp(dss, hostkey, 11))
        return LIBSSH2_HOSTKEY_TYPE_DSS;
#endif

    if(len < 15)
        return LIBSSH2_HOSTKEY_TYPE_UNKNOWN;

    if(!memcmp(ed25519, hostkey, 15))
        return LIBSSH2_HOSTKEY_TYPE_ED25519;

    if(len < 23)
        return LIBSSH2_HOSTKEY_TYPE_UNKNOWN;

    if(!memcmp(ecdsa_256, hostkey, 23))
        return LIBSSH2_HOSTKEY_TYPE_ECDSA_256;

    if(!memcmp(ecdsa_384, hostkey, 23))
        return LIBSSH2_HOSTKEY_TYPE_ECDSA_384;

    if(!memcmp(ecdsa_521, hostkey, 23))
        return LIBSSH2_HOSTKEY_TYPE_ECDSA_521;

    return LIBSSH2_HOSTKEY_TYPE_UNKNOWN;
}

/*
 * libssh2_session_hostkey
 *
 * Returns the server key and length.
 *
 */
LIBSSH2_API const char *
libssh2_session_hostkey(LIBSSH2_SESSION *session, size_t *len, int *type)
{
    if(session->server_hostkey_len) {
        if(len)
            *len = session->server_hostkey_len;
        if(type)
            *type = hostkey_type(session->server_hostkey,
                                 session->server_hostkey_len);
        return (char *) session->server_hostkey;
    }
    if(len)
        *len = 0;
    return NULL;
}
