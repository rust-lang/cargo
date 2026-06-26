#ifndef LIBSSH2_WINCNG_H
#define LIBSSH2_WINCNG_H
/*
 * Copyright (C) Marc Hoersken <info@marc-hoersken.de>
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

#define LIBSSH2_CRYPTO_ENGINE libssh2_wincng

/* required for cross-compilation against the w64 mingw-runtime package */
#if defined(_WIN32_WINNT) && (_WIN32_WINNT < 0x0600)
#undef _WIN32_WINNT
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif

#include <windows.h>
#include <bcrypt.h>

#define LIBSSH2_MD5 1

#define LIBSSH2_HMAC_RIPEMD 0
#define LIBSSH2_HMAC_SHA256 1
#define LIBSSH2_HMAC_SHA512 1

#define LIBSSH2_AES_CBC 1
#define LIBSSH2_AES_CTR 1
#define LIBSSH2_AES_GCM 0
#define LIBSSH2_BLOWFISH 0
#define LIBSSH2_RC4 1
#define LIBSSH2_CAST 0
#define LIBSSH2_3DES 1

#define LIBSSH2_RSA 1
#define LIBSSH2_RSA_SHA1 1
#define LIBSSH2_RSA_SHA2 1
#define LIBSSH2_DSA 1
#define LIBSSH2_ED25519 0

/*
 * Conditionally enable ECDSA support.
 *
 * ECDSA support requires the use of
 *
 *   BCryptDeriveKey(..., BCRYPT_KDF_RAW_SECRET, ... )
 *
 * This functionality is only available as of Windows 10. To maintain
 * backward compatibility, ECDSA support is therefore disabled
 * by default and needs to be explicitly enabled using a build
 * flag.
 */
#ifdef LIBSSH2_ECDSA_WINCNG
#define LIBSSH2_ECDSA 1
#else
#define LIBSSH2_ECDSA 0
#endif

#include "crypto_config.h"

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define MD5_DIGEST_LENGTH 16
#endif
#define SHA_DIGEST_LENGTH 20
#define SHA256_DIGEST_LENGTH 32
#define SHA384_DIGEST_LENGTH 48
#define SHA512_DIGEST_LENGTH 64

#define EC_MAX_POINT_LEN ((528 * 2 / 8) + 1)

#if LIBSSH2_ECDSA
#else
#define _libssh2_ec_key void
#endif

/*******************************************************************/
/*
 * Windows CNG backend: Global context handles
 */

struct _libssh2_wincng_ctx {
    BCRYPT_ALG_HANDLE hAlgRNG;
    BCRYPT_ALG_HANDLE hAlgHashMD5;
    BCRYPT_ALG_HANDLE hAlgHashSHA1;
    BCRYPT_ALG_HANDLE hAlgHashSHA256;
    BCRYPT_ALG_HANDLE hAlgHashSHA384;
    BCRYPT_ALG_HANDLE hAlgHashSHA512;
    BCRYPT_ALG_HANDLE hAlgHmacMD5;
    BCRYPT_ALG_HANDLE hAlgHmacSHA1;
    BCRYPT_ALG_HANDLE hAlgHmacSHA256;
    BCRYPT_ALG_HANDLE hAlgHmacSHA384;
    BCRYPT_ALG_HANDLE hAlgHmacSHA512;
    BCRYPT_ALG_HANDLE hAlgRSA;
    BCRYPT_ALG_HANDLE hAlgDSA;
    BCRYPT_ALG_HANDLE hAlgAES_CBC;
    BCRYPT_ALG_HANDLE hAlgAES_ECB;
    BCRYPT_ALG_HANDLE hAlgRC4_NA;
    BCRYPT_ALG_HANDLE hAlg3DES_CBC;
    BCRYPT_ALG_HANDLE hAlgDH;
    BCRYPT_ALG_HANDLE hAlgChacha20;
#if LIBSSH2_ECDSA
    BCRYPT_ALG_HANDLE hAlgECDH[3];  /* indexed by libssh2_curve_type */
    BCRYPT_ALG_HANDLE hAlgECDSA[3]; /* indexed by libssh2_curve_type */
#endif
    volatile int hasAlgDHwithKDF; /* -1=no, 0=maybe, 1=yes */
};

extern struct _libssh2_wincng_ctx _libssh2_wincng;


/*******************************************************************/
/*
 * Windows CNG backend: Generic functions
 */

#define libssh2_crypto_init() \
    _libssh2_wincng_init()
#define libssh2_crypto_exit() \
    _libssh2_wincng_free()

#define _libssh2_random(buf, len) \
    _libssh2_wincng_random(buf, len)

#define libssh2_prepare_iovec(vec, len)  /* Empty. */


/*******************************************************************/
/*
 * Windows CNG backend: Hash structure
 */

typedef struct __libssh2_wincng_hash_ctx {
    BCRYPT_HASH_HANDLE hHash;
    unsigned char *pbHashObject;
    ULONG dwHashObject;
    ULONG cbHash;
} _libssh2_wincng_hash_ctx;

/*
 * Windows CNG backend: Hash functions
 */

#define libssh2_sha1_ctx _libssh2_wincng_hash_ctx
#define libssh2_sha1_init(ctx) \
    (_libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHashSHA1, \
                               SHA_DIGEST_LENGTH, NULL, 0) == 0)
#define libssh2_sha1_update(ctx, data, datalen) \
    (_libssh2_wincng_hash_update(&ctx, data, (ULONG) datalen) == 0)
#define libssh2_sha1_final(ctx, hash) \
    (_libssh2_wincng_hash_final(&ctx, hash) == 0)
#define libssh2_sha1(data, datalen, hash) \
    _libssh2_wincng_hash(data, datalen, _libssh2_wincng.hAlgHashSHA1, \
                         hash, SHA_DIGEST_LENGTH)

#define libssh2_sha256_ctx _libssh2_wincng_hash_ctx
#define libssh2_sha256_init(ctx) \
    (_libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHashSHA256, \
                               SHA256_DIGEST_LENGTH, NULL, 0) == 0)
#define libssh2_sha256_update(ctx, data, datalen) \
    (_libssh2_wincng_hash_update(&ctx, data, (ULONG) datalen) == 0)
#define libssh2_sha256_final(ctx, hash) \
    (_libssh2_wincng_hash_final(&ctx, hash) == 0)
#define libssh2_sha256(data, datalen, hash) \
    _libssh2_wincng_hash(data, datalen, _libssh2_wincng.hAlgHashSHA256, \
                         hash, SHA256_DIGEST_LENGTH)

#define libssh2_sha384_ctx _libssh2_wincng_hash_ctx
#define libssh2_sha384_init(ctx) \
    (_libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHashSHA384, \
                               SHA384_DIGEST_LENGTH, NULL, 0) == 0)
#define libssh2_sha384_update(ctx, data, datalen) \
    (_libssh2_wincng_hash_update(&ctx, data, (ULONG) datalen) == 0)
#define libssh2_sha384_final(ctx, hash) \
    (_libssh2_wincng_hash_final(&ctx, hash) == 0)
#define libssh2_sha384(data, datalen, hash) \
    _libssh2_wincng_hash(data, datalen, _libssh2_wincng.hAlgHashSHA384, \
                         hash, SHA384_DIGEST_LENGTH)

#define libssh2_sha512_ctx _libssh2_wincng_hash_ctx
#define libssh2_sha512_init(ctx) \
    (_libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHashSHA512, \
                               SHA512_DIGEST_LENGTH, NULL, 0) == 0)
#define libssh2_sha512_update(ctx, data, datalen) \
    (_libssh2_wincng_hash_update(&ctx, data, (ULONG) datalen) == 0)
#define libssh2_sha512_final(ctx, hash) \
    (_libssh2_wincng_hash_final(&ctx, hash) == 0)
#define libssh2_sha512(data, datalen, hash) \
    _libssh2_wincng_hash(data, datalen, _libssh2_wincng.hAlgHashSHA512, \
                         hash, SHA512_DIGEST_LENGTH)

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define libssh2_md5_ctx _libssh2_wincng_hash_ctx
#define libssh2_md5_init(ctx) \
    (_libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHashMD5, \
                               MD5_DIGEST_LENGTH, NULL, 0) == 0)
#define libssh2_md5_update(ctx, data, datalen) \
    (_libssh2_wincng_hash_update(&ctx, data, (ULONG) datalen) == 0)
#define libssh2_md5_final(ctx, hash) \
    (_libssh2_wincng_hash_final(&ctx, hash) == 0)
#endif

/*
 * Windows CNG backend: HMAC functions
 */

#define libssh2_hmac_ctx _libssh2_wincng_hash_ctx


/*******************************************************************/
/*
 * Windows CNG backend: Key Context structure
 */

typedef struct __libssh2_wincng_key_ctx {
    BCRYPT_KEY_HANDLE hKey;
    void *pbKeyObject;
    DWORD cbKeyObject;
} _libssh2_wincng_key_ctx;


/*
 * Windows CNG backend: RSA functions
 */

#define libssh2_rsa_ctx _libssh2_wincng_key_ctx
#define _libssh2_rsa_new(rsactx, e, e_len, n, n_len, \
                         d, d_len, p, p_len, q, q_len, \
                         e1, e1_len, e2, e2_len, c, c_len) \
    _libssh2_wincng_rsa_new(rsactx, e, e_len, n, n_len, \
                            d, d_len, p, p_len, q, q_len, \
                            e1, e1_len, e2, e2_len, c, c_len)
#define _libssh2_rsa_new_private(rsactx, s, filename, passphrase) \
    _libssh2_wincng_rsa_new_private(rsactx, s, filename, passphrase)
#define _libssh2_rsa_new_private_frommemory(rsactx, s, filedata, \
                                            filedata_len, passphrase) \
    _libssh2_wincng_rsa_new_private_frommemory(rsactx, s, filedata, \
                                               filedata_len, passphrase)
#define _libssh2_rsa_sha1_sign(s, rsactx, hash, hash_len, sig, sig_len) \
    _libssh2_wincng_rsa_sha1_sign(s, rsactx, hash, hash_len, sig, sig_len)
#define _libssh2_rsa_sha2_sign(s, rsactx, hash, hash_len, sig, sig_len) \
    _libssh2_wincng_rsa_sha2_sign(s, rsactx, hash, hash_len, sig, sig_len)
#define _libssh2_rsa_sha1_verify(rsactx, sig, sig_len, m, m_len) \
    _libssh2_wincng_rsa_sha1_verify(rsactx, sig, sig_len, m, m_len)
#define _libssh2_rsa_sha2_verify(rsactx, hash_len, sig, sig_len, m, m_len) \
    _libssh2_wincng_rsa_sha2_verify(rsactx, hash_len, sig, sig_len, m, m_len)
#define _libssh2_rsa_free(rsactx) \
    _libssh2_wincng_rsa_free(rsactx)

/*
 * Windows CNG backend: DSA functions
 */

#define libssh2_dsa_ctx _libssh2_wincng_key_ctx
#define _libssh2_dsa_new(dsactx, p, p_len, q, q_len, \
                         g, g_len, y, y_len, x, x_len) \
    _libssh2_wincng_dsa_new(dsactx, p, p_len, q, q_len, \
                            g, g_len, y, y_len, x, x_len)
#define _libssh2_dsa_new_private(dsactx, s, filename, passphrase) \
    _libssh2_wincng_dsa_new_private(dsactx, s, filename, passphrase)
#define _libssh2_dsa_new_private_frommemory(dsactx, s, filedata, \
                                            filedata_len, passphrase) \
    _libssh2_wincng_dsa_new_private_frommemory(dsactx, s, filedata, \
                                               filedata_len, passphrase)
#define _libssh2_dsa_sha1_sign(dsactx, hash, hash_len, sig) \
    _libssh2_wincng_dsa_sha1_sign(dsactx, hash, hash_len, sig)
#define _libssh2_dsa_sha1_verify(dsactx, sig, m, m_len) \
    _libssh2_wincng_dsa_sha1_verify(dsactx, sig, m, m_len)
#define _libssh2_dsa_free(dsactx) \
    _libssh2_wincng_dsa_free(dsactx)


/*
 * Windows CNG backend: ECDSA functions
 */

typedef enum {
    LIBSSH2_EC_CURVE_NISTP256 = 0,
    LIBSSH2_EC_CURVE_NISTP384 = 1,
    LIBSSH2_EC_CURVE_NISTP521 = 2,
} libssh2_curve_type;

typedef struct __libssh2_wincng_ecdsa_ctx {
    BCRYPT_KEY_HANDLE handle;
    libssh2_curve_type curve;
} _libssh2_wincng_ecdsa_key;

#define libssh2_ecdsa_ctx _libssh2_wincng_ecdsa_key

#if LIBSSH2_ECDSA
#define _libssh2_ec_key _libssh2_wincng_ecdsa_key
#endif

void
_libssh2_wincng_ecdsa_free(libssh2_ecdsa_ctx* ctx);

#define _libssh2_ecdsa_create_key(session, privkey, pubkey_octal, \
                                  pubkey_octal_len, curve) \
    _libssh2_wincng_ecdh_create_key(session, privkey, pubkey_octal, \
                                    pubkey_octal_len, curve)

#define _libssh2_ecdsa_curve_name_with_octal_new(ctx, k, k_len, curve) \
    _libssh2_wincng_ecdsa_curve_name_with_octal_new(ctx, k, k_len, curve)

#define _libssh2_ecdh_gen_k(k, privkey, server_pubkey, server_pubkey_len) \
    _libssh2_wincng_ecdh_gen_k(k, privkey, server_pubkey, server_pubkey_len)

#define _libssh2_ecdsa_verify(ctx, r, r_len, s, s_len, m, m_len) \
    _libssh2_wincng_ecdsa_verify(ctx, r, r_len, s, s_len, m, m_len)

#define _libssh2_ecdsa_new_private(ctx, session, filename, passphrase) \
    _libssh2_wincng_ecdsa_new_private(ctx, session, filename, passphrase)

#define _libssh2_ecdsa_new_private_frommemory(ctx, session, filedata, \
                                              filedata_len, passphrase) \
    _libssh2_wincng_ecdsa_new_private_frommemory(ctx, session, filedata, \
                                                 filedata_len, passphrase)

#define _libssh2_ecdsa_sign(session, ctx, hash, hash_len, sign, sign_len) \
    _libssh2_wincng_ecdsa_sign(session, ctx, hash, hash_len, sign, sign_len)

#define _libssh2_ecdsa_get_curve_type(ctx) \
    _libssh2_wincng_ecdsa_get_curve_type(ctx)

#define _libssh2_ecdsa_free(ecdsactx) \
    _libssh2_wincng_ecdsa_free(ecdsactx)


/*
 * Windows CNG backend: Key functions
 */

#define _libssh2_pub_priv_keyfile(s, m, m_len, p, p_len, pk, pw) \
    _libssh2_wincng_pub_priv_keyfile(s, m, m_len, p, p_len, pk, pw)
#define _libssh2_pub_priv_keyfilememory(s, m, m_len, p, p_len, \
                                        pk, pk_len, pw) \
    _libssh2_wincng_pub_priv_keyfilememory(s, m, m_len, p, p_len, \
                                           pk, pk_len, pw)
#define _libssh2_sk_pub_keyfilememory(s, m, m_len, p, p_len, alg, app, \
                                      f, kh, kh_len, pk, pk_len, pw) \
    _libssh2_wincng_sk_pub_keyfilememory(s, m, m_len, p, p_len, alg, app, \
                                         f, kh, kh_len, pk, pk_len, pw)

/*******************************************************************/
/*
 * Windows CNG backend: Cipher Context structure
 */

struct _libssh2_wincng_cipher_ctx {
    BCRYPT_KEY_HANDLE hKey;
    unsigned char *pbKeyObject;
    unsigned char *pbIV;
    unsigned char *pbCtr;
    ULONG dwKeyObject;
    ULONG dwIV;
    ULONG dwBlockLength;
    ULONG dwCtrLength;
};

#define _libssh2_cipher_ctx struct _libssh2_wincng_cipher_ctx

/*
 * Windows CNG backend: Cipher Type structure
 */

struct _libssh2_wincng_cipher_type {
    BCRYPT_ALG_HANDLE *phAlg;
    ULONG dwKeyLength;
    int useIV;      /* TODO: Convert to bool when a C89 compatible bool type
                       is defined */
    int ctrMode;
};

#define _libssh2_cipher_type(type) struct _libssh2_wincng_cipher_type type

#define _libssh2_cipher_aes256ctr { &_libssh2_wincng.hAlgAES_ECB, 32, 0, 1 }
#define _libssh2_cipher_aes192ctr { &_libssh2_wincng.hAlgAES_ECB, 24, 0, 1 }
#define _libssh2_cipher_aes128ctr { &_libssh2_wincng.hAlgAES_ECB, 16, 0, 1 }
#define _libssh2_cipher_aes256    { &_libssh2_wincng.hAlgAES_CBC, 32, 1, 0 }
#define _libssh2_cipher_aes192    { &_libssh2_wincng.hAlgAES_CBC, 24, 1, 0 }
#define _libssh2_cipher_aes128    { &_libssh2_wincng.hAlgAES_CBC, 16, 1, 0 }
#define _libssh2_cipher_arcfour   { &_libssh2_wincng.hAlgRC4_NA, 16, 0, 0 }
#define _libssh2_cipher_3des      { &_libssh2_wincng.hAlg3DES_CBC, 24, 1, 0 }
#define _libssh2_cipher_chacha20  { &_libssh2_wincng.hAlgChacha20, 24, 1, 0 }

/*
 * Windows CNG backend: Cipher functions
 */

#define _libssh2_cipher_init(ctx, type, iv, secret, encrypt) \
    _libssh2_wincng_cipher_init(ctx, type, iv, secret, encrypt)
#define _libssh2_cipher_crypt(ctx, type, encrypt, block, blocklen, fl) \
    _libssh2_wincng_cipher_crypt(ctx, type, encrypt, block, blocklen, fl)
#define _libssh2_cipher_dtor(ctx) \
    _libssh2_wincng_cipher_dtor(ctx)

/*******************************************************************/
/*
 * Windows CNG backend: BigNumber Context
 */

#define _libssh2_bn_ctx int /* not used */
#define _libssh2_bn_ctx_new() 0 /* not used */
#define _libssh2_bn_ctx_free(bnctx) ((void)0) /* not used */


/*******************************************************************/
/*
 * Windows CNG backend: BigNumber structure
 */

struct _libssh2_wincng_bignum {
    unsigned char *bignum;
    ULONG length;
};

#define _libssh2_bn struct _libssh2_wincng_bignum

/*
 * Windows CNG backend: BigNumber functions
 */

#define _libssh2_bn_init() \
    _libssh2_wincng_bignum_init()
#define _libssh2_bn_init_from_bin() \
    _libssh2_bn_init()
#define _libssh2_bn_set_word(bn, word) \
    _libssh2_wincng_bignum_set_word(bn, word)
#define _libssh2_bn_from_bin(bn, len, bin) \
    _libssh2_wincng_bignum_from_bin(bn, (ULONG) len, bin)
#define _libssh2_bn_to_bin(bn, bin) \
    _libssh2_wincng_bignum_to_bin(bn, bin)
#define _libssh2_bn_bytes(bn) bn->length
#define _libssh2_bn_bits(bn) \
    _libssh2_wincng_bignum_bits(bn)
#define _libssh2_bn_free(bn) \
    _libssh2_wincng_bignum_free(bn)

/*
 * Windows CNG backend: Diffie-Hellman support
 */

/* Default generate and safe prime sizes for
   diffie-hellman-group-exchange-sha1 */
#define LIBSSH2_DH_GEX_MINGROUP     2048
#define LIBSSH2_DH_GEX_OPTGROUP     4096
#define LIBSSH2_DH_GEX_MAXGROUP     4096

#define LIBSSH2_DH_MAX_MODULUS_BITS 16384

typedef struct {
    /* holds our private and public key components */
    BCRYPT_KEY_HANDLE dh_handle;
    /* records the parsed out modulus and generator
     * parameters that are shared  with the peer */
    BCRYPT_DH_PARAMETER_HEADER *dh_params;
    /* records the parsed out private key component for
     * fallback if the DH API raw KDF is not supported */
    struct _libssh2_wincng_bignum *dh_privbn;
} _libssh2_dh_ctx;

#define libssh2_dh_init(dhctx) _libssh2_dh_init(dhctx)
#define libssh2_dh_key_pair(dhctx, public, g, p, group_order, bnctx) \
    _libssh2_dh_key_pair(dhctx, public, g, p, group_order)
#define libssh2_dh_secret(dhctx, secret, f, p, bnctx) \
    _libssh2_dh_secret(dhctx, secret, f, p)
#define libssh2_dh_dtor(dhctx) _libssh2_dh_dtor(dhctx)

/*******************************************************************/
/*
 * Windows CNG backend: forward declarations
 */
void _libssh2_wincng_init(void);
void _libssh2_wincng_free(void);
int _libssh2_wincng_random(void *buf, size_t len);

int
_libssh2_wincng_hash_init(_libssh2_wincng_hash_ctx *ctx,
                          BCRYPT_ALG_HANDLE hAlg, ULONG hashlen,
                          unsigned char *key, ULONG keylen);
int
_libssh2_wincng_hash_update(_libssh2_wincng_hash_ctx *ctx,
                            const void *data, ULONG datalen);
int
_libssh2_wincng_hash_final(_libssh2_wincng_hash_ctx *ctx,
                           unsigned char *hash);
int
_libssh2_wincng_hash(const unsigned char *data, ULONG datalen,
                     BCRYPT_ALG_HANDLE hAlg,
                     unsigned char *hash, ULONG hashlen);

void
_libssh2_wincng_rsa_free(libssh2_rsa_ctx *rsa);

#if LIBSSH2_DSA
void
_libssh2_wincng_dsa_free(libssh2_dsa_ctx *dsa);
#endif

void
_libssh2_wincng_cipher_dtor(_libssh2_cipher_ctx *ctx);

_libssh2_bn *
_libssh2_wincng_bignum_init(void);
int
_libssh2_wincng_bignum_set_word(_libssh2_bn *bn, ULONG word);
ULONG
_libssh2_wincng_bignum_bits(const _libssh2_bn *bn);
int
_libssh2_wincng_bignum_from_bin(_libssh2_bn *bn, ULONG len,
                                const unsigned char *bin);
int
_libssh2_wincng_bignum_to_bin(const _libssh2_bn *bn, unsigned char *bin);
void
_libssh2_wincng_bignum_free(_libssh2_bn *bn);
extern void
_libssh2_dh_init(_libssh2_dh_ctx *dhctx);
extern int
_libssh2_dh_key_pair(_libssh2_dh_ctx *dhctx, _libssh2_bn *public,
                     _libssh2_bn *g, _libssh2_bn *p, int group_order);
extern int
_libssh2_dh_secret(_libssh2_dh_ctx *dhctx, _libssh2_bn *secret,
                   _libssh2_bn *f, _libssh2_bn *p);
extern void
_libssh2_dh_dtor(_libssh2_dh_ctx *dhctx);

#endif /* LIBSSH2_WINCNG_H */
