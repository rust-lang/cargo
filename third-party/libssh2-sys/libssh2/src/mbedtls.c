/* Copyright (C) Art <https://github.com/wildart>
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

#ifdef LIBSSH2_CRYPTO_C /* Compile this via crypto.c */

#include <stdlib.h>

#if MBEDTLS_VERSION_NUMBER < 0x03000000
#define mbedtls_cipher_info_get_key_bitlen(c) (c->key_bitlen)
#define mbedtls_cipher_info_get_iv_size(c)    (c->iv_size)
#define mbedtls_rsa_get_len(rsa)              (rsa->len)

#define MBEDTLS_PRIVATE(m) m
#endif

/*******************************************************************/
/*
 * mbedTLS backend: Global context handles
 */

static mbedtls_entropy_context  _libssh2_mbedtls_entropy;
static mbedtls_ctr_drbg_context _libssh2_mbedtls_ctr_drbg;

/*******************************************************************/
/*
 * mbedTLS backend: Generic functions
 */

void
_libssh2_mbedtls_init(void)
{
    int ret;

    mbedtls_entropy_init(&_libssh2_mbedtls_entropy);
    mbedtls_ctr_drbg_init(&_libssh2_mbedtls_ctr_drbg);

    ret = mbedtls_ctr_drbg_seed(&_libssh2_mbedtls_ctr_drbg,
                                mbedtls_entropy_func,
                                &_libssh2_mbedtls_entropy, NULL, 0);
    if(ret)
        mbedtls_ctr_drbg_free(&_libssh2_mbedtls_ctr_drbg);
}

void
_libssh2_mbedtls_free(void)
{
    mbedtls_ctr_drbg_free(&_libssh2_mbedtls_ctr_drbg);
    mbedtls_entropy_free(&_libssh2_mbedtls_entropy);
}

int
_libssh2_mbedtls_random(unsigned char *buf, size_t len)
{
    int ret;
    ret = mbedtls_ctr_drbg_random(&_libssh2_mbedtls_ctr_drbg, buf, len);
    return ret == 0 ? 0 : -1;
}

static void
_libssh2_mbedtls_safe_free(void *buf, size_t len)
{
    if(!buf)
        return;

    if(len > 0)
        _libssh2_explicit_zero(buf, len);

    mbedtls_free(buf);
}

int
_libssh2_mbedtls_cipher_init(_libssh2_cipher_ctx *ctx,
                             _libssh2_cipher_type(algo),
                             unsigned char *iv,
                             unsigned char *secret,
                             int encrypt)
{
    const mbedtls_cipher_info_t *cipher_info;
    int ret, op;

    if(!ctx)
        return -1;

    op = encrypt == 0 ? MBEDTLS_ENCRYPT : MBEDTLS_DECRYPT;

    cipher_info = mbedtls_cipher_info_from_type(algo);
    if(!cipher_info)
        return -1;

    mbedtls_cipher_init(ctx);
    ret = mbedtls_cipher_setup(ctx, cipher_info);
    if(!ret)
        ret = mbedtls_cipher_setkey(ctx,
                  secret,
                  (int)mbedtls_cipher_info_get_key_bitlen(cipher_info),
                  op);

    if(!ret)
        ret = mbedtls_cipher_set_iv(ctx, iv,
                  mbedtls_cipher_info_get_iv_size(cipher_info));

    return ret == 0 ? 0 : -1;
}

int
_libssh2_mbedtls_cipher_crypt(_libssh2_cipher_ctx *ctx,
                              _libssh2_cipher_type(algo),
                              int encrypt,
                              unsigned char *block,
                              size_t blocklen, int firstlast)
{
    int ret;
    unsigned char *output;
    size_t osize, olen, finish_olen;

    (void)encrypt;
    (void)algo;
    (void)firstlast;

    osize = blocklen + mbedtls_cipher_get_block_size(ctx);

    output = (unsigned char *)mbedtls_calloc(osize, sizeof(char));
    if(output) {
        ret = mbedtls_cipher_reset(ctx);

        if(!ret)
            ret = mbedtls_cipher_update(ctx, block, blocklen, output, &olen);

        if(!ret)
            ret = mbedtls_cipher_finish(ctx, output + olen, &finish_olen);

        if(!ret) {
            olen += finish_olen;
            memcpy(block, output, olen);
        }

        _libssh2_mbedtls_safe_free(output, osize);
    }
    else
        ret = -1;

    return ret == 0 ? 0 : -1;
}

void
_libssh2_mbedtls_cipher_dtor(_libssh2_cipher_ctx *ctx)
{
    mbedtls_cipher_free(ctx);
}


int
_libssh2_mbedtls_hash_init(mbedtls_md_context_t *ctx,
                           mbedtls_md_type_t mdtype,
                           const unsigned char *key, size_t keylen)
{
    const mbedtls_md_info_t *md_info;
    int ret, hmac;

    md_info = mbedtls_md_info_from_type(mdtype);
    if(!md_info)
        return 0;

    hmac = key ? 1 : 0;

    mbedtls_md_init(ctx);
    ret = mbedtls_md_setup(ctx, md_info, hmac);
    if(!ret) {
        if(hmac)
            ret = mbedtls_md_hmac_starts(ctx, key, keylen);
        else
            ret = mbedtls_md_starts(ctx);
    }

    return ret == 0 ? 1 : 0;
}

int
_libssh2_mbedtls_hash_final(mbedtls_md_context_t *ctx, unsigned char *hash)
{
    int ret;

    ret = mbedtls_md_finish(ctx, hash);
    mbedtls_md_free(ctx);

    return ret == 0 ? 1 : 0;
}

int
_libssh2_mbedtls_hash(const unsigned char *data, size_t datalen,
                      mbedtls_md_type_t mdtype, unsigned char *hash)
{
    const mbedtls_md_info_t *md_info;
    int ret;

    md_info = mbedtls_md_info_from_type(mdtype);
    if(!md_info)
        return 0;

    ret = mbedtls_md(md_info, data, datalen, hash);

    return ret == 0 ? 0 : -1;
}

int _libssh2_hmac_ctx_init(libssh2_hmac_ctx *ctx)
{
    memset(ctx, 0, sizeof(*ctx));
    return 1;
}

#if LIBSSH2_MD5
int _libssh2_hmac_md5_init(libssh2_hmac_ctx *ctx,
                           void *key, size_t keylen)
{
    return _libssh2_mbedtls_hash_init(ctx, MBEDTLS_MD_MD5, key, keylen);
}
#endif

#if LIBSSH2_HMAC_RIPEMD
int _libssh2_hmac_ripemd160_init(libssh2_hmac_ctx *ctx,
                                 void *key, size_t keylen)
{
    return _libssh2_mbedtls_hash_init(ctx, MBEDTLS_MD_RIPEMD160, key, keylen);
}
#endif

int _libssh2_hmac_sha1_init(libssh2_hmac_ctx *ctx,
                            void *key, size_t keylen)
{
    return _libssh2_mbedtls_hash_init(ctx, MBEDTLS_MD_SHA1, key, keylen);
}

int _libssh2_hmac_sha256_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    return _libssh2_mbedtls_hash_init(ctx, MBEDTLS_MD_SHA256, key, keylen);
}

int _libssh2_hmac_sha512_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    return _libssh2_mbedtls_hash_init(ctx, MBEDTLS_MD_SHA512, key, keylen);
}

int _libssh2_hmac_update(libssh2_hmac_ctx *ctx,
                         const void *data, size_t datalen)
{
    int ret = mbedtls_md_hmac_update(ctx, data, datalen);

    return ret == 0 ? 1 : 0;
}

int _libssh2_hmac_final(libssh2_hmac_ctx *ctx, void *data)
{
    int ret = mbedtls_md_hmac_finish(ctx, data);

    return ret == 0 ? 1 : 0;
}

void _libssh2_hmac_cleanup(libssh2_hmac_ctx *ctx)
{
    mbedtls_md_free(ctx);
}

/*******************************************************************/
/*
 * mbedTLS backend: BigNumber functions
 */

_libssh2_bn *
_libssh2_mbedtls_bignum_init(void)
{
    _libssh2_bn *bignum;

    bignum = (_libssh2_bn *)mbedtls_calloc(1, sizeof(_libssh2_bn));
    if(bignum) {
        mbedtls_mpi_init(bignum);
    }

    return bignum;
}

void
_libssh2_mbedtls_bignum_free(_libssh2_bn *bn)
{
    if(bn) {
        mbedtls_mpi_free(bn);
        mbedtls_free(bn);
    }
}

static int
_libssh2_mbedtls_bignum_random(_libssh2_bn *bn, int bits, int top, int bottom)
{
    size_t len;
    int err;
    size_t i;

    if(!bn || bits <= 0)
        return -1;

    len = (bits + 7) >> 3;
    err = mbedtls_mpi_fill_random(bn, len, mbedtls_ctr_drbg_random,
                                  &_libssh2_mbedtls_ctr_drbg);
    if(err)
        return -1;

    /* Zero unused bits above the most significant bit */
    for(i = len*8 - 1; (size_t)bits <= i; --i) {
        err = mbedtls_mpi_set_bit(bn, i, 0);
        if(err)
            return -1;
    }

    /* If `top` is -1, the most significant bit of the random number can be
       zero.  If top is 0, the most significant bit of the random number is
       set to 1, and if top is 1, the two most significant bits of the number
       will be set to 1, so that the product of two such random numbers will
       always have 2*bits length.
    */
    if(top >= 0) {
        for(i = 0; i <= (size_t)top; ++i) {
            err = mbedtls_mpi_set_bit(bn, bits-i-1, 1);
            if(err)
                return -1;
        }
    }

    /* make odd by setting first bit in least significant byte */
    if(bottom) {
        err = mbedtls_mpi_set_bit(bn, 0, 1);
        if(err)
            return -1;
    }

    return 0;
}


/*******************************************************************/
/*
 * mbedTLS backend: RSA functions
 */

int
_libssh2_mbedtls_rsa_new(libssh2_rsa_ctx **rsa,
                         const unsigned char *edata,
                         unsigned long elen,
                         const unsigned char *ndata,
                         unsigned long nlen,
                         const unsigned char *ddata,
                         unsigned long dlen,
                         const unsigned char *pdata,
                         unsigned long plen,
                         const unsigned char *qdata,
                         unsigned long qlen,
                         const unsigned char *e1data,
                         unsigned long e1len,
                         const unsigned char *e2data,
                         unsigned long e2len,
                         const unsigned char *coeffdata,
                         unsigned long coefflen)
{
    int ret;
    libssh2_rsa_ctx *ctx;

    ctx = (libssh2_rsa_ctx *) mbedtls_calloc(1, sizeof(libssh2_rsa_ctx));
    if(ctx) {
#if MBEDTLS_VERSION_NUMBER >= 0x03000000
        mbedtls_rsa_init(ctx);
#else
        mbedtls_rsa_init(ctx, MBEDTLS_RSA_PKCS_V15, 0);
#endif
    }
    else
        return -1;

    /* !checksrc! disable ASSIGNWITHINCONDITION 1 */
    if((ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(E)),
                                      edata, elen)) ||
       (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(N)),
                                      ndata, nlen))) {
        ret = -1;
    }

    if(!ret) {
        ctx->MBEDTLS_PRIVATE(len) =
            mbedtls_mpi_size(&(ctx->MBEDTLS_PRIVATE(N)));
    }

    if(!ret && ddata) {
        /* !checksrc! disable ASSIGNWITHINCONDITION 1 */
        if((ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(D)),
                                          ddata, dlen)) ||
           (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(P)),
                                          pdata, plen)) ||
           (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(Q)),
                                          qdata, qlen)) ||
           (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(DP)),
                                          e1data, e1len)) ||
           (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(DQ)),
                                          e2data, e2len)) ||
           (ret = mbedtls_mpi_read_binary(&(ctx->MBEDTLS_PRIVATE(QP)),
                                          coeffdata, coefflen))) {
            ret = -1;
        }
        ret = mbedtls_rsa_check_privkey(ctx);
    }
    else if(!ret) {
        ret = mbedtls_rsa_check_pubkey(ctx);
    }

    if(ret && ctx) {
        _libssh2_mbedtls_rsa_free(ctx);
        ctx = NULL;
    }
    *rsa = ctx;
    return ret;
}

int
_libssh2_mbedtls_rsa_new_private(libssh2_rsa_ctx **rsa,
                                 LIBSSH2_SESSION *session,
                                 const char *filename,
                                 const unsigned char *passphrase)
{
    int ret;
    mbedtls_pk_context pkey;
    mbedtls_rsa_context *pk_rsa;

    *rsa = (libssh2_rsa_ctx *) LIBSSH2_ALLOC(session, sizeof(libssh2_rsa_ctx));
    if(!*rsa)
        return -1;

#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    mbedtls_rsa_init(*rsa);
#else
    mbedtls_rsa_init(*rsa, MBEDTLS_RSA_PKCS_V15, 0);
#endif
    mbedtls_pk_init(&pkey);

#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    ret = mbedtls_pk_parse_keyfile(&pkey, filename, (char *)passphrase,
                                   mbedtls_ctr_drbg_random,
                                   &_libssh2_mbedtls_ctr_drbg);
#else
    ret = mbedtls_pk_parse_keyfile(&pkey, filename, (char *)passphrase);
#endif
    if(ret || mbedtls_pk_get_type(&pkey) != MBEDTLS_PK_RSA) {
        mbedtls_pk_free(&pkey);
        mbedtls_rsa_free(*rsa);
        LIBSSH2_FREE(session, *rsa);
        *rsa = NULL;
        return -1;
    }

    pk_rsa = mbedtls_pk_rsa(pkey);
    mbedtls_rsa_copy(*rsa, pk_rsa);
    mbedtls_pk_free(&pkey);

    return 0;
}

int
_libssh2_mbedtls_rsa_new_private_frommemory(libssh2_rsa_ctx **rsa,
                                            LIBSSH2_SESSION *session,
                                            const char *filedata,
                                            size_t filedata_len,
                                            unsigned const char *passphrase)
{
    int ret;
    mbedtls_pk_context pkey;
    mbedtls_rsa_context *pk_rsa;
    void *filedata_nullterm;
    size_t pwd_len;

    *rsa = (libssh2_rsa_ctx *) mbedtls_calloc(1, sizeof(libssh2_rsa_ctx));
    if(!*rsa)
        return -1;

    /*
    mbedtls checks in "mbedtls/pkparse.c:1184" if "key[keylen - 1] != '\0'"
    private-key from memory will fail if the last byte is not a null byte
    */
    filedata_nullterm = mbedtls_calloc(filedata_len + 1, 1);
    if(!filedata_nullterm) {
        return -1;
    }
    memcpy(filedata_nullterm, filedata, filedata_len);

    mbedtls_pk_init(&pkey);

    pwd_len = passphrase ? strlen((const char *)passphrase) : 0;
#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    ret = mbedtls_pk_parse_key(&pkey, (unsigned char *)filedata_nullterm,
                               filedata_len + 1,
                               passphrase, pwd_len,
                               mbedtls_ctr_drbg_random,
                               &_libssh2_mbedtls_ctr_drbg);
#else
    ret = mbedtls_pk_parse_key(&pkey, (unsigned char *)filedata_nullterm,
                               filedata_len + 1,
                               passphrase, pwd_len);
#endif
    _libssh2_mbedtls_safe_free(filedata_nullterm, filedata_len);

    if(ret || mbedtls_pk_get_type(&pkey) != MBEDTLS_PK_RSA) {
        mbedtls_pk_free(&pkey);
        mbedtls_rsa_free(*rsa);
        LIBSSH2_FREE(session, *rsa);
        *rsa = NULL;
        return -1;
    }

    pk_rsa = mbedtls_pk_rsa(pkey);
    mbedtls_rsa_copy(*rsa, pk_rsa);
    mbedtls_pk_free(&pkey);

    return 0;
}

int
_libssh2_mbedtls_rsa_sha2_verify(libssh2_rsa_ctx * rsactx,
                                 size_t hash_len,
                                 const unsigned char *sig,
                                 size_t sig_len,
                                 const unsigned char *m,
                                 size_t m_len)
{
    int ret;
    int md_type;
    unsigned char *hash;

    if(sig_len < mbedtls_rsa_get_len(rsactx))
        return -1;

    hash = malloc(hash_len);
    if(!hash)
        return -1;

    if(hash_len == SHA_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA1;
    }
    else if(hash_len == SHA256_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA256;
    }
    else if(hash_len == SHA512_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA512;
    }
    else{
        free(hash);
        return -1; /* unsupported digest */
    }
    ret = _libssh2_mbedtls_hash(m, m_len, md_type, hash);

    if(ret) {
        free(hash);
        return -1; /* failure */
    }

#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    ret = mbedtls_rsa_pkcs1_verify(rsactx,
                                   md_type, (unsigned int)hash_len,
                                   hash, sig);
#else
    ret = mbedtls_rsa_pkcs1_verify(rsactx, NULL, NULL, MBEDTLS_RSA_PUBLIC,
                                   md_type, (unsigned int)hash_len,
                                   hash, sig);
#endif
    free(hash);

    return (ret == 0) ? 0 : -1;
}

int
_libssh2_mbedtls_rsa_sha1_verify(libssh2_rsa_ctx * rsactx,
                                 const unsigned char *sig,
                                 size_t sig_len,
                                 const unsigned char *m,
                                 size_t m_len)
{
    return _libssh2_mbedtls_rsa_sha2_verify(rsactx, SHA_DIGEST_LENGTH,
                                            sig, sig_len, m, m_len);
}

int
_libssh2_mbedtls_rsa_sha2_sign(LIBSSH2_SESSION *session,
                               libssh2_rsa_ctx *rsa,
                               const unsigned char *hash,
                               size_t hash_len,
                               unsigned char **signature,
                               size_t *signature_len)
{
    int ret;
    unsigned char *sig;
    size_t sig_len;
    int md_type;

    sig_len = mbedtls_rsa_get_len(rsa);
    sig = LIBSSH2_ALLOC(session, sig_len);
    if(!sig) {
        return -1;
    }
    ret = 0;
    if(hash_len == SHA_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA1;
    }
    else if(hash_len == SHA256_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA256;
    }
    else if(hash_len == SHA512_DIGEST_LENGTH) {
        md_type = MBEDTLS_MD_SHA512;
    }
    else {
        _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                       "Unsupported hash digest length");
        ret = -1;
    }
    if(ret == 0) {
#if MBEDTLS_VERSION_NUMBER >= 0x03000000
        ret = mbedtls_rsa_pkcs1_sign(rsa,
                                     mbedtls_ctr_drbg_random,
                                     &_libssh2_mbedtls_ctr_drbg,
                                     md_type, (unsigned int)hash_len,
                                     hash, sig);
#else
        ret = mbedtls_rsa_pkcs1_sign(rsa, NULL, NULL, MBEDTLS_RSA_PRIVATE,
                                     md_type, (unsigned int)hash_len,
                                     hash, sig);
#endif
    }
    if(ret) {
        LIBSSH2_FREE(session, sig);
        return -1;
    }

    *signature = sig;
    *signature_len = sig_len;

    return (ret == 0) ? 0 : -1;
}

int
_libssh2_mbedtls_rsa_sha1_sign(LIBSSH2_SESSION * session,
                               libssh2_rsa_ctx * rsactx,
                               const unsigned char *hash,
                               size_t hash_len,
                               unsigned char **signature,
                               size_t *signature_len)
{
    return _libssh2_mbedtls_rsa_sha2_sign(session, rsactx, hash, hash_len,
                                          signature, signature_len);
}

void
_libssh2_mbedtls_rsa_free(libssh2_rsa_ctx *ctx)
{
    mbedtls_rsa_free(ctx);
    mbedtls_free(ctx);
}

static unsigned char *
gen_publickey_from_rsa(LIBSSH2_SESSION *session,
                       mbedtls_rsa_context *rsa,
                       size_t *keylen)
{
    uint32_t e_bytes, n_bytes;
    uint32_t len;
    unsigned char *key;
    unsigned char *p;

    e_bytes = (uint32_t)mbedtls_mpi_size(&rsa->MBEDTLS_PRIVATE(E));
    n_bytes = (uint32_t)mbedtls_mpi_size(&rsa->MBEDTLS_PRIVATE(N));

    /* Key form is "ssh-rsa" + e + n. */
    len = 4 + 7 + 4 + e_bytes + 4 + n_bytes;

    key = LIBSSH2_ALLOC(session, len);
    if(!key) {
        return NULL;
    }

    /* Process key encoding. */
    p = key;

    _libssh2_htonu32(p, 7);  /* Key type. */
    p += 4;
    memcpy(p, "ssh-rsa", 7);
    p += 7;

    _libssh2_htonu32(p, e_bytes);
    p += 4;
    mbedtls_mpi_write_binary(&rsa->MBEDTLS_PRIVATE(E), p, e_bytes);

    _libssh2_htonu32(p, n_bytes);
    p += 4;
    mbedtls_mpi_write_binary(&rsa->MBEDTLS_PRIVATE(N), p, n_bytes);

    *keylen = (size_t)(p - key);
    return key;
}

static int
_libssh2_mbedtls_pub_priv_key(LIBSSH2_SESSION *session,
                              unsigned char **method,
                              size_t *method_len,
                              unsigned char **pubkeydata,
                              size_t *pubkeydata_len,
                              mbedtls_pk_context *pkey)
{
    unsigned char *key = NULL, *mth = NULL;
    size_t keylen = 0, mthlen = 0;
    int ret;
    mbedtls_rsa_context *rsa;

    if(mbedtls_pk_get_type(pkey) != MBEDTLS_PK_RSA) {
        mbedtls_pk_free(pkey);
        return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                              "Key type not supported");
    }

    /* write method */
    mthlen = 7;
    mth = LIBSSH2_ALLOC(session, mthlen);
    if(mth) {
        memcpy(mth, "ssh-rsa", mthlen);
    }
    else {
        ret = -1;
    }

    rsa = mbedtls_pk_rsa(*pkey);
    key = gen_publickey_from_rsa(session, rsa, &keylen);
    if(!key) {
        ret = -1;
    }

    /* write output */
    if(ret) {
        if(mth)
            LIBSSH2_FREE(session, mth);
        if(key)
            LIBSSH2_FREE(session, key);
    }
    else {
        *method = mth;
        *method_len = mthlen;
        *pubkeydata = key;
        *pubkeydata_len = keylen;
    }

    return ret;
}

int
_libssh2_mbedtls_pub_priv_keyfile(LIBSSH2_SESSION *session,
                                  unsigned char **method,
                                  size_t *method_len,
                                  unsigned char **pubkeydata,
                                  size_t *pubkeydata_len,
                                  const char *privatekey,
                                  const char *passphrase)
{
    mbedtls_pk_context pkey;
    char buf[1024];
    int ret;

    mbedtls_pk_init(&pkey);
#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    ret = mbedtls_pk_parse_keyfile(&pkey, privatekey, passphrase,
                                   mbedtls_ctr_drbg_random,
                                   &_libssh2_mbedtls_ctr_drbg);
#else
    ret = mbedtls_pk_parse_keyfile(&pkey, privatekey, passphrase);
#endif
    if(ret) {
        mbedtls_strerror(ret, (char *)buf, sizeof(buf));
        mbedtls_pk_free(&pkey);
        return _libssh2_error(session, LIBSSH2_ERROR_FILE, buf);
    }

    ret = _libssh2_mbedtls_pub_priv_key(session, method, method_len,
                                        pubkeydata, pubkeydata_len, &pkey);

    mbedtls_pk_free(&pkey);

    return ret;
}

int
_libssh2_mbedtls_pub_priv_keyfilememory(LIBSSH2_SESSION *session,
                                        unsigned char **method,
                                        size_t *method_len,
                                        unsigned char **pubkeydata,
                                        size_t *pubkeydata_len,
                                        const char *privatekeydata,
                                        size_t privatekeydata_len,
                                        const char *passphrase)
{
    mbedtls_pk_context pkey;
    char buf[1024];
    int ret;
    void *privatekeydata_nullterm;
    size_t pwd_len;

    /*
    mbedtls checks in "mbedtls/pkparse.c:1184" if "key[keylen - 1] != '\0'"
    private-key from memory will fail if the last byte is not a null byte
    */
    privatekeydata_nullterm = mbedtls_calloc(privatekeydata_len + 1, 1);
    if(!privatekeydata_nullterm) {
        return -1;
    }
    memcpy(privatekeydata_nullterm, privatekeydata, privatekeydata_len);

    mbedtls_pk_init(&pkey);

    pwd_len = passphrase ? strlen((const char *)passphrase) : 0;
#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    ret = mbedtls_pk_parse_key(&pkey,
                               (unsigned char *)privatekeydata_nullterm,
                               privatekeydata_len + 1,
                               (const unsigned char *)passphrase, pwd_len,
                               mbedtls_ctr_drbg_random,
                               &_libssh2_mbedtls_ctr_drbg);
#else
    ret = mbedtls_pk_parse_key(&pkey,
                               (unsigned char *)privatekeydata_nullterm,
                               privatekeydata_len + 1,
                               (const unsigned char *)passphrase, pwd_len);
#endif
    _libssh2_mbedtls_safe_free(privatekeydata_nullterm, privatekeydata_len);

    if(ret) {
        mbedtls_strerror(ret, (char *)buf, sizeof(buf));
        mbedtls_pk_free(&pkey);
        return _libssh2_error(session, LIBSSH2_ERROR_FILE, buf);
    }

    ret = _libssh2_mbedtls_pub_priv_key(session, method, method_len,
                                        pubkeydata, pubkeydata_len, &pkey);

    mbedtls_pk_free(&pkey);

    return ret;
}

int
_libssh2_mbedtls_sk_pub_keyfilememory(LIBSSH2_SESSION *session,
                                      unsigned char **method,
                                      size_t *method_len,
                                      unsigned char **pubkeydata,
                                      size_t *pubkeydata_len,
                                      int *algorithm,
                                      unsigned char *flags,
                                      const char **application,
                                      const unsigned char **key_handle,
                                      size_t *handle_len,
                                      const char *privatekeydata,
                                      size_t privatekeydata_len,
                                      const char *passphrase)
{
    (void)method;
    (void)method_len;
    (void)pubkeydata;
    (void)pubkeydata_len;
    (void)algorithm;
    (void)flags;
    (void)application;
    (void)key_handle;
    (void)handle_len;
    (void)privatekeydata;
    (void)privatekeydata_len;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                    "Unable to extract public SK key from private key file: "
                    "Method unimplemented in mbedTLS backend");
}

void _libssh2_init_aes_ctr(void)
{
    /* no implementation */
}


/*******************************************************************/
/*
 * mbedTLS backend: Diffie-Hellman functions
 */

void
_libssh2_dh_init(_libssh2_dh_ctx *dhctx)
{
    *dhctx = _libssh2_mbedtls_bignum_init();    /* Random from client */
}

int
_libssh2_dh_key_pair(_libssh2_dh_ctx *dhctx, _libssh2_bn *public,
                     _libssh2_bn *g, _libssh2_bn *p, int group_order)
{
    /* Generate x and e */
    _libssh2_mbedtls_bignum_random(*dhctx, group_order * 8 - 1, 0, -1);
    mbedtls_mpi_exp_mod(public, g, *dhctx, p, NULL);
    return 0;
}

int
_libssh2_dh_secret(_libssh2_dh_ctx *dhctx, _libssh2_bn *secret,
                   _libssh2_bn *f, _libssh2_bn *p)
{
    /* Compute the shared secret */
    mbedtls_mpi_exp_mod(secret, f, *dhctx, p, NULL);
    return 0;
}

void
_libssh2_dh_dtor(_libssh2_dh_ctx *dhctx)
{
    _libssh2_mbedtls_bignum_free(*dhctx);
    *dhctx = NULL;
}

#if LIBSSH2_ECDSA

/*******************************************************************/
/*
 * mbedTLS backend: ECDSA functions
 */

/*
 * _libssh2_ecdsa_create_key
 *
 * Creates a local private key based on input curve
 * and returns octal value and octal length
 *
 */

int
_libssh2_mbedtls_ecdsa_create_key(LIBSSH2_SESSION *session,
                                  _libssh2_ec_key **privkey,
                                  unsigned char **pubkey_oct,
                                  size_t *pubkey_oct_len,
                                  libssh2_curve_type curve)
{
    size_t plen = 0;

    *privkey = LIBSSH2_ALLOC(session, sizeof(mbedtls_ecp_keypair));

    if(!*privkey)
        goto failed;

    mbedtls_ecdsa_init(*privkey);

    if(mbedtls_ecdsa_genkey(*privkey, (mbedtls_ecp_group_id)curve,
                            mbedtls_ctr_drbg_random,
                            &_libssh2_mbedtls_ctr_drbg))
        goto failed;

    plen = 2 * mbedtls_mpi_size(&(*privkey)->MBEDTLS_PRIVATE(grp).P) + 1;
    *pubkey_oct = LIBSSH2_ALLOC(session, plen);

    if(!*pubkey_oct)
        goto failed;

    if(mbedtls_ecp_point_write_binary(&(*privkey)->MBEDTLS_PRIVATE(grp),
                                      &(*privkey)->MBEDTLS_PRIVATE(Q),
                                      MBEDTLS_ECP_PF_UNCOMPRESSED,
                                      pubkey_oct_len, *pubkey_oct, plen) == 0)
        return 0;

failed:

    _libssh2_mbedtls_ecdsa_free(*privkey);
    _libssh2_mbedtls_safe_free(*pubkey_oct, plen);
    *privkey = NULL;

    return -1;
}

/* _libssh2_ecdsa_curve_name_with_octal_new
 *
 * Creates a new public key given an octal string, length and type
 *
 */

int
_libssh2_mbedtls_ecdsa_curve_name_with_octal_new(libssh2_ecdsa_ctx **ctx,
                                                 const unsigned char *k,
                                                 size_t k_len,
                                                 libssh2_curve_type curve)
{
    *ctx = mbedtls_calloc(1, sizeof(mbedtls_ecp_keypair));

    if(!*ctx)
        goto failed;

    mbedtls_ecdsa_init(*ctx);

    if(mbedtls_ecp_group_load(&(*ctx)->MBEDTLS_PRIVATE(grp),
                              (mbedtls_ecp_group_id)curve))
        goto failed;

    if(mbedtls_ecp_point_read_binary(&(*ctx)->MBEDTLS_PRIVATE(grp),
                                     &(*ctx)->MBEDTLS_PRIVATE(Q),
                                     k, k_len))
        goto failed;

    if(mbedtls_ecp_check_pubkey(&(*ctx)->MBEDTLS_PRIVATE(grp),
                                &(*ctx)->MBEDTLS_PRIVATE(Q)) == 0)
        return 0;

failed:

    _libssh2_mbedtls_ecdsa_free(*ctx);
    *ctx = NULL;

    return -1;
}

/* _libssh2_ecdh_gen_k
 *
 * Computes the shared secret K given a local private key,
 * remote public key and length
 */

int
_libssh2_mbedtls_ecdh_gen_k(_libssh2_bn **k,
                            _libssh2_ec_key *privkey,
                            const unsigned char *server_pubkey,
                            size_t server_pubkey_len)
{
    mbedtls_ecp_point pubkey;
    int rc = 0;

    if(!*k)
        return -1;

    mbedtls_ecp_point_init(&pubkey);

    if(mbedtls_ecp_point_read_binary(&privkey->MBEDTLS_PRIVATE(grp),
                                     &pubkey,
                                     server_pubkey, server_pubkey_len)) {
        rc = -1;
        goto cleanup;
    }

    if(mbedtls_ecdh_compute_shared(&privkey->MBEDTLS_PRIVATE(grp), *k,
                                   &pubkey,
                                   &privkey->MBEDTLS_PRIVATE(d),
                                   mbedtls_ctr_drbg_random,
                                   &_libssh2_mbedtls_ctr_drbg)) {
        rc = -1;
        goto cleanup;
    }

    if(mbedtls_ecp_check_privkey(&privkey->MBEDTLS_PRIVATE(grp), *k))
        rc = -1;

cleanup:

    mbedtls_ecp_point_free(&pubkey);

    return rc;
}

#define LIBSSH2_MBEDTLS_ECDSA_VERIFY(digest_type)                           \
    do {                                                                    \
        unsigned char hsh[SHA##digest_type##_DIGEST_LENGTH];                \
                                                                            \
        if(libssh2_sha##digest_type(m, m_len, hsh) == 0) {                  \
            rc = mbedtls_ecdsa_verify(&ctx->MBEDTLS_PRIVATE(grp), hsh,      \
                                      SHA##digest_type##_DIGEST_LENGTH,     \
                                      &ctx->MBEDTLS_PRIVATE(Q), &pr, &ps);  \
        }                                                                   \
    } while(0)

/* _libssh2_ecdsa_verify
 *
 * Verifies the ECDSA signature of a hashed message
 *
 */

int
_libssh2_mbedtls_ecdsa_verify(libssh2_ecdsa_ctx *ctx,
                              const unsigned char *r, size_t r_len,
                              const unsigned char *s, size_t s_len,
                              const unsigned char *m, size_t m_len)
{
    mbedtls_mpi pr, ps;
    int rc = -1;

    mbedtls_mpi_init(&pr);
    mbedtls_mpi_init(&ps);

    if(mbedtls_mpi_read_binary(&pr, r, r_len))
        goto cleanup;

    if(mbedtls_mpi_read_binary(&ps, s, s_len))
        goto cleanup;

    switch(_libssh2_ecdsa_get_curve_type(ctx)) {
    case LIBSSH2_EC_CURVE_NISTP256:
        LIBSSH2_MBEDTLS_ECDSA_VERIFY(256);
        break;
    case LIBSSH2_EC_CURVE_NISTP384:
        LIBSSH2_MBEDTLS_ECDSA_VERIFY(384);
        break;
    case LIBSSH2_EC_CURVE_NISTP521:
        LIBSSH2_MBEDTLS_ECDSA_VERIFY(512);
        break;
    default:
        rc = -1;
    }

cleanup:

    mbedtls_mpi_free(&pr);
    mbedtls_mpi_free(&ps);

    return (rc == 0) ? 0 : -1;
}

static int
_libssh2_mbedtls_parse_eckey(libssh2_ecdsa_ctx **ctx,
                             mbedtls_pk_context *pkey,
                             LIBSSH2_SESSION *session,
                             const unsigned char *data,
                             size_t data_len,
                             const unsigned char *pwd)
{
    size_t pwd_len;

    pwd_len = pwd ? strlen((const char *) pwd) : 0;

#if MBEDTLS_VERSION_NUMBER >= 0x03000000
    if(mbedtls_pk_parse_key(pkey, data, data_len, pwd, pwd_len,
                            mbedtls_ctr_drbg_random,
                            &_libssh2_mbedtls_ctr_drbg))

        goto failed;
#else
    if(mbedtls_pk_parse_key(pkey, data, data_len, pwd, pwd_len))
        goto failed;
#endif

    if(mbedtls_pk_get_type(pkey) != MBEDTLS_PK_ECKEY)
        goto failed;

    *ctx = LIBSSH2_ALLOC(session, sizeof(libssh2_ecdsa_ctx));

    if(!*ctx)
        goto failed;

    mbedtls_ecdsa_init(*ctx);

    if(mbedtls_ecdsa_from_keypair(*ctx, mbedtls_pk_ec(*pkey)) == 0)
        return 0;

failed:

    _libssh2_mbedtls_ecdsa_free(*ctx);
    *ctx = NULL;

    return -1;
}

static int
_libssh2_mbedtls_parse_openssh_key(libssh2_ecdsa_ctx **ctx,
                                   LIBSSH2_SESSION *session,
                                   const unsigned char *data,
                                   size_t data_len,
                                   const unsigned char *pwd)
{
    libssh2_curve_type type;
    unsigned char *name = NULL;
    struct string_buf *decrypted = NULL;
    size_t curvelen, exponentlen, pointlen;
    unsigned char *curve, *exponent, *point_buf;

    if(_libssh2_openssh_pem_parse_memory(session, pwd,
                                         (const char *)data, data_len,
                                         &decrypted))
        goto failed;

    if(_libssh2_get_string(decrypted, &name, NULL))
        goto failed;

    if(_libssh2_mbedtls_ecdsa_curve_type_from_name((const char *)name,
                                                   &type))
        goto failed;

    if(_libssh2_get_string(decrypted, &curve, &curvelen))
        goto failed;

    if(_libssh2_get_string(decrypted, &point_buf, &pointlen))
        goto failed;

    if(_libssh2_get_bignum_bytes(decrypted, &exponent, &exponentlen))
        goto failed;

    *ctx = LIBSSH2_ALLOC(session, sizeof(libssh2_ecdsa_ctx));

    if(!*ctx)
        goto failed;

    mbedtls_ecdsa_init(*ctx);

    if(mbedtls_ecp_group_load(&(*ctx)->MBEDTLS_PRIVATE(grp),
                              (mbedtls_ecp_group_id)type))
        goto failed;

    if(mbedtls_mpi_read_binary(&(*ctx)->MBEDTLS_PRIVATE(d),
                               exponent, exponentlen))
        goto failed;

    if(mbedtls_ecp_mul(&(*ctx)->MBEDTLS_PRIVATE(grp),
                       &(*ctx)->MBEDTLS_PRIVATE(Q),
                       &(*ctx)->MBEDTLS_PRIVATE(d),
                       &(*ctx)->MBEDTLS_PRIVATE(grp).G,
                       mbedtls_ctr_drbg_random,
                       &_libssh2_mbedtls_ctr_drbg))
        goto failed;

    if(mbedtls_ecp_check_privkey(&(*ctx)->MBEDTLS_PRIVATE(grp),
                                 &(*ctx)->MBEDTLS_PRIVATE(d)) == 0)
        goto cleanup;

failed:

    _libssh2_mbedtls_ecdsa_free(*ctx);
    *ctx = NULL;

cleanup:

    if(decrypted) {
        _libssh2_string_buf_free(session, decrypted);
    }

    return *ctx ? 0 : -1;
}

/* Force-expose internal mbedTLS function */
#if MBEDTLS_VERSION_NUMBER >= 0x03060000
int mbedtls_pk_load_file(const char *path, unsigned char **buf, size_t *n);
#endif

/* _libssh2_ecdsa_new_private
 *
 * Creates a new private key given a file path and password
 *
 */

int
_libssh2_mbedtls_ecdsa_new_private(libssh2_ecdsa_ctx **ctx,
                                   LIBSSH2_SESSION *session,
                                   const char *filename,
                                   const unsigned char *pwd)
{
    mbedtls_pk_context pkey;
    unsigned char *data = NULL;
    size_t data_len = 0;

    mbedtls_pk_init(&pkey);

    /* FIXME: Reimplement this functionality via a public API. */
    if(mbedtls_pk_load_file(filename, &data, &data_len))
        goto cleanup;

    if(_libssh2_mbedtls_parse_eckey(ctx, &pkey, session,
                                    data, data_len, pwd) == 0)
        goto cleanup;

    _libssh2_mbedtls_parse_openssh_key(ctx, session, data, data_len, pwd);

cleanup:

    mbedtls_pk_free(&pkey);

    _libssh2_mbedtls_safe_free(data, data_len);

    return *ctx ? 0 : -1;
}

/* _libssh2_ecdsa_new_private
 *
 * Creates a new private key given a file data and password
 *
 */

int
_libssh2_mbedtls_ecdsa_new_private_frommemory(libssh2_ecdsa_ctx **ctx,
                                              LIBSSH2_SESSION *session,
                                              const char *data,
                                              size_t data_len,
                                              const unsigned char *pwd)
{
    unsigned char *ntdata;
    mbedtls_pk_context pkey;

    mbedtls_pk_init(&pkey);

    ntdata = LIBSSH2_ALLOC(session, data_len + 1);

    if(!ntdata)
        goto cleanup;

    memcpy(ntdata, data, data_len);

    if(_libssh2_mbedtls_parse_eckey(ctx, &pkey, session,
                                    ntdata, data_len + 1, pwd) == 0)
        goto cleanup;

    _libssh2_mbedtls_parse_openssh_key(ctx, session,
                                       ntdata, data_len + 1, pwd);

cleanup:

    mbedtls_pk_free(&pkey);

    _libssh2_mbedtls_safe_free(ntdata, data_len);

    return *ctx ? 0 : -1;
}

static unsigned char *
_libssh2_mbedtls_mpi_write_binary(unsigned char *buf,
                                  const mbedtls_mpi *mpi,
                                  size_t bytes)
{
    unsigned char *p = buf;
    uint32_t size = (uint32_t)bytes;

    if(sizeof(&p) / sizeof(p[0]) < 4) {
        goto done;
    }

    p += 4;
    *p = 0;

    if(size > 0) {
        mbedtls_mpi_write_binary(mpi, p + 1, size - 1);
    }

    if(size > 0 && !(*(p + 1) & 0x80)) {
        memmove(p, p + 1, --size);
    }

    _libssh2_htonu32(p - 4, size);

done:

    return p + size;
}

/* _libssh2_ecdsa_sign
 *
 * Computes the ECDSA signature of a previously-hashed message
 *
 */

int
_libssh2_mbedtls_ecdsa_sign(LIBSSH2_SESSION *session,
                            libssh2_ecdsa_ctx *ctx,
                            const unsigned char *hash,
                            size_t hash_len,
                            unsigned char **sign,
                            size_t *sign_len)
{
    size_t r_len, s_len, tmp_sign_len = 0;
    unsigned char *sp, *tmp_sign = NULL;
    mbedtls_mpi pr, ps;

    mbedtls_mpi_init(&pr);
    mbedtls_mpi_init(&ps);

    if(mbedtls_ecdsa_sign(&ctx->MBEDTLS_PRIVATE(grp), &pr, &ps,
                          &ctx->MBEDTLS_PRIVATE(d),
                          hash, hash_len,
                          mbedtls_ctr_drbg_random,
                          &_libssh2_mbedtls_ctr_drbg))
        goto cleanup;

    r_len = mbedtls_mpi_size(&pr) + 1;
    s_len = mbedtls_mpi_size(&ps) + 1;
    tmp_sign_len = r_len + s_len + 8;

    tmp_sign = LIBSSH2_CALLOC(session, tmp_sign_len);

    if(!tmp_sign)
        goto cleanup;

    sp = tmp_sign;
    sp = _libssh2_mbedtls_mpi_write_binary(sp, &pr, r_len);
    sp = _libssh2_mbedtls_mpi_write_binary(sp, &ps, s_len);

    *sign_len = (size_t)(sp - tmp_sign);

    *sign = LIBSSH2_CALLOC(session, *sign_len);

    if(!*sign)
        goto cleanup;

    memcpy(*sign, tmp_sign, *sign_len);

cleanup:

    mbedtls_mpi_free(&pr);
    mbedtls_mpi_free(&ps);

    _libssh2_mbedtls_safe_free(tmp_sign, tmp_sign_len);

    return *sign ? 0 : -1;
}

/* _libssh2_ecdsa_get_curve_type
 *
 * returns key curve type that maps to libssh2_curve_type
 *
 */

libssh2_curve_type
_libssh2_mbedtls_ecdsa_get_curve_type(libssh2_ecdsa_ctx *ctx)
{
    return (libssh2_curve_type) ctx->MBEDTLS_PRIVATE(grp).id;
}

/* _libssh2_ecdsa_curve_type_from_name
 *
 * returns 0 for success, key curve type that maps to libssh2_curve_type
 *
 */

int
_libssh2_mbedtls_ecdsa_curve_type_from_name(const char *name,
                                            libssh2_curve_type *out_type)
{
    int ret = 0;
    libssh2_curve_type type;

    if(!name || strlen(name) != 19)
        return -1;

    if(strcmp(name, "ecdsa-sha2-nistp256") == 0)
        type = LIBSSH2_EC_CURVE_NISTP256;
    else if(strcmp(name, "ecdsa-sha2-nistp384") == 0)
        type = LIBSSH2_EC_CURVE_NISTP384;
    else if(strcmp(name, "ecdsa-sha2-nistp521") == 0)
        type = LIBSSH2_EC_CURVE_NISTP521;
    else {
        ret = -1;
    }

    if(ret == 0 && out_type) {
        *out_type = type;
    }

    return ret;
}

void
_libssh2_mbedtls_ecdsa_free(libssh2_ecdsa_ctx *ctx)
{
    mbedtls_ecdsa_free(ctx);
    mbedtls_free(ctx);
}
#endif /* LIBSSH2_ECDSA */


/* _libssh2_supported_key_sign_algorithms
 *
 * Return supported key hash algo upgrades, see crypto.h
 *
 */

const char *
_libssh2_supported_key_sign_algorithms(LIBSSH2_SESSION *session,
                                       unsigned char *key_method,
                                       size_t key_method_len)
{
    (void)session;

#if LIBSSH2_RSA_SHA2
    if(key_method_len == 7 &&
       memcmp(key_method, "ssh-rsa", key_method_len) == 0) {
        return "rsa-sha2-512,rsa-sha2-256"
#if LIBSSH2_RSA_SHA1
            ",ssh-rsa"
#endif
            ;
    }
#endif

    return NULL;
}

#endif /* LIBSSH2_CRYPTO_C */
