#ifndef LIBSSH2_MBEDTLS_H
#define LIBSSH2_MBEDTLS_H
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

#define LIBSSH2_CRYPTO_ENGINE libssh2_mbedtls

#ifdef __GNUC__
#pragma GCC diagnostic push
/* mbedTLS (as of v3.5.1) has a `[-Werror=arith-conversion]`
   warning in its public headers. */
#if !defined(__clang__) && __GNUC__ >= 10
#pragma GCC diagnostic ignored "-Warith-conversion"
#endif
#if defined(__clang__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif
/* mbedTLS (as of v3.5.1) has a duplicate function declaration
   in its public headers. Disable the warning that detects it. */
#pragma GCC diagnostic ignored "-Wredundant-decls"
#endif

#include <mbedtls/version.h>
#include <mbedtls/platform.h>
#include <mbedtls/md.h>
#include <mbedtls/rsa.h>
#include <mbedtls/bignum.h>
#include <mbedtls/cipher.h>
#ifdef MBEDTLS_ECDH_C
# include <mbedtls/ecdh.h>
#endif
#ifdef MBEDTLS_ECDSA_C
# include <mbedtls/ecdsa.h>
#endif
#include <mbedtls/entropy.h>
#include <mbedtls/ctr_drbg.h>
#include <mbedtls/pk.h>
#include <mbedtls/error.h>

#ifdef __GNUC__
#pragma GCC diagnostic pop
#endif

/* Define which features are supported. */
#define LIBSSH2_MD5             1

#define LIBSSH2_HMAC_RIPEMD     1
#define LIBSSH2_HMAC_SHA256     1
#define LIBSSH2_HMAC_SHA512     1

#define LIBSSH2_AES_CBC         1
#define LIBSSH2_AES_CTR         1
#define LIBSSH2_AES_GCM         0
#ifdef MBEDTLS_CIPHER_BLOWFISH_CBC
# define LIBSSH2_BLOWFISH       1
#else
# define LIBSSH2_BLOWFISH       0
#endif
#ifdef MBEDTLS_CIPHER_ARC4_128
# define LIBSSH2_RC4            1
#else
# define LIBSSH2_RC4            0
#endif
#define LIBSSH2_CAST            0
#define LIBSSH2_3DES            1

#define LIBSSH2_RSA             1
#define LIBSSH2_RSA_SHA1        1
#define LIBSSH2_RSA_SHA2        1
#define LIBSSH2_DSA             0
#ifdef MBEDTLS_ECDSA_C
# define LIBSSH2_ECDSA          1
#else
# define LIBSSH2_ECDSA          0
#endif
#define LIBSSH2_ED25519         0

#include "crypto_config.h"

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define MD5_DIGEST_LENGTH      16
#endif
#define SHA_DIGEST_LENGTH      20
#define SHA256_DIGEST_LENGTH   32
#define SHA384_DIGEST_LENGTH   48
#define SHA512_DIGEST_LENGTH   64

#define EC_MAX_POINT_LEN ((528 * 2 / 8) + 1)


/*******************************************************************/
/*
 * mbedTLS backend: Generic functions
 */

#define libssh2_crypto_init() \
    _libssh2_mbedtls_init()
#define libssh2_crypto_exit() \
    _libssh2_mbedtls_free()

#define _libssh2_random(buf, len) \
    _libssh2_mbedtls_random(buf, len)

#define libssh2_prepare_iovec(vec, len)  /* Empty. */


/*******************************************************************/
/*
 * mbedTLS backend: HMAC functions
 */

#define libssh2_hmac_ctx    mbedtls_md_context_t


/*******************************************************************/
/*
 * mbedTLS backend: SHA1 functions
 */

#define libssh2_sha1_ctx      mbedtls_md_context_t

#define libssh2_sha1_init(pctx) \
    _libssh2_mbedtls_hash_init(pctx, MBEDTLS_MD_SHA1, NULL, 0)
#define libssh2_sha1_update(ctx, data, datalen) \
    (mbedtls_md_update(&ctx, (const unsigned char *) data, datalen) == 0)
#define libssh2_sha1_final(ctx, hash) \
    _libssh2_mbedtls_hash_final(&ctx, hash)
#define libssh2_sha1(data, datalen, hash) \
    _libssh2_mbedtls_hash(data, datalen, MBEDTLS_MD_SHA1, hash)


/*******************************************************************/
/*
 * mbedTLS backend: SHA256 functions
 */

#define libssh2_sha256_ctx      mbedtls_md_context_t

#define libssh2_sha256_init(pctx) \
    _libssh2_mbedtls_hash_init(pctx, MBEDTLS_MD_SHA256, NULL, 0)
#define libssh2_sha256_update(ctx, data, datalen) \
    (mbedtls_md_update(&ctx, (const unsigned char *) data, datalen) == 0)
#define libssh2_sha256_final(ctx, hash) \
    _libssh2_mbedtls_hash_final(&ctx, hash)
#define libssh2_sha256(data, datalen, hash) \
    _libssh2_mbedtls_hash(data, datalen, MBEDTLS_MD_SHA256, hash)


/*******************************************************************/
/*
 * mbedTLS backend: SHA384 functions
 */

#define libssh2_sha384_ctx      mbedtls_md_context_t

#define libssh2_sha384_init(pctx) \
    _libssh2_mbedtls_hash_init(pctx, MBEDTLS_MD_SHA384, NULL, 0)
#define libssh2_sha384_update(ctx, data, datalen) \
    (mbedtls_md_update(&ctx, (const unsigned char *) data, datalen) == 0)
#define libssh2_sha384_final(ctx, hash) \
    _libssh2_mbedtls_hash_final(&ctx, hash)
#define libssh2_sha384(data, datalen, hash) \
    _libssh2_mbedtls_hash(data, datalen, MBEDTLS_MD_SHA384, hash)


/*******************************************************************/
/*
 * mbedTLS backend: SHA512 functions
 */

#define libssh2_sha512_ctx      mbedtls_md_context_t

#define libssh2_sha512_init(pctx) \
    _libssh2_mbedtls_hash_init(pctx, MBEDTLS_MD_SHA512, NULL, 0)
#define libssh2_sha512_update(ctx, data, datalen) \
    (mbedtls_md_update(&ctx, (const unsigned char *) data, datalen) == 0)
#define libssh2_sha512_final(ctx, hash) \
    _libssh2_mbedtls_hash_final(&ctx, hash)
#define libssh2_sha512(data, datalen, hash) \
    _libssh2_mbedtls_hash(data, datalen, MBEDTLS_MD_SHA512, hash)


/*******************************************************************/
/*
 * mbedTLS backend: MD5 functions
 */

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define libssh2_md5_ctx      mbedtls_md_context_t

#define libssh2_md5_init(pctx) \
    _libssh2_mbedtls_hash_init(pctx, MBEDTLS_MD_MD5, NULL, 0)
#define libssh2_md5_update(ctx, data, datalen) \
    (mbedtls_md_update(&ctx, (const unsigned char *) data, datalen) == 0)
#define libssh2_md5_final(ctx, hash) \
    _libssh2_mbedtls_hash_final(&ctx, hash)
#endif

/*******************************************************************/
/*
 * mbedTLS backend: RSA functions
 */

#define libssh2_rsa_ctx  mbedtls_rsa_context

#define _libssh2_rsa_new(rsactx, e, e_len, n, n_len, \
                         d, d_len, p, p_len, q, q_len, \
                         e1, e1_len, e2, e2_len, c, c_len) \
    _libssh2_mbedtls_rsa_new(rsactx, e, e_len, n, n_len, \
                             d, d_len, p, p_len, q, q_len, \
                             e1, e1_len, e2, e2_len, c, c_len)

#define _libssh2_rsa_new_private(rsactx, s, filename, passphrase) \
    _libssh2_mbedtls_rsa_new_private(rsactx, s, filename, passphrase)

#define _libssh2_rsa_new_private_frommemory(rsactx, s, filedata, \
                                            filedata_len, passphrase) \
    _libssh2_mbedtls_rsa_new_private_frommemory(rsactx, s, filedata, \
                                                filedata_len, passphrase)

#define _libssh2_rsa_sha1_sign(s, rsactx, hash, hash_len, sig, sig_len) \
    _libssh2_mbedtls_rsa_sha1_sign(s, rsactx, hash, hash_len, sig, sig_len)

#define _libssh2_rsa_sha2_sign(s, rsactx, hash, hash_len, sig, sig_len) \
    _libssh2_mbedtls_rsa_sha2_sign(s, rsactx, hash, hash_len, sig, sig_len)


#define _libssh2_rsa_sha1_verify(rsactx, sig, sig_len, m, m_len) \
    _libssh2_mbedtls_rsa_sha1_verify(rsactx, sig, sig_len, m, m_len)

#define _libssh2_rsa_sha2_verify(rsactx, hash_len, sig, sig_len, m, m_len) \
    _libssh2_mbedtls_rsa_sha2_verify(rsactx, hash_len, sig, sig_len, m, m_len)

#define _libssh2_rsa_free(rsactx) \
    _libssh2_mbedtls_rsa_free(rsactx)


/*******************************************************************/
/*
 * mbedTLS backend: ECDSA structures
 */

#if LIBSSH2_ECDSA

typedef enum {
#ifdef MBEDTLS_ECP_DP_SECP256R1_ENABLED
    LIBSSH2_EC_CURVE_NISTP256 = MBEDTLS_ECP_DP_SECP256R1,
#else
    LIBSSH2_EC_CURVE_NISTP256 = MBEDTLS_ECP_DP_NONE,
#endif
#ifdef MBEDTLS_ECP_DP_SECP384R1_ENABLED
    LIBSSH2_EC_CURVE_NISTP384 = MBEDTLS_ECP_DP_SECP384R1,
#else
    LIBSSH2_EC_CURVE_NISTP384 = MBEDTLS_ECP_DP_NONE,
#endif
#ifdef MBEDTLS_ECP_DP_SECP521R1_ENABLED
    LIBSSH2_EC_CURVE_NISTP521 = MBEDTLS_ECP_DP_SECP521R1
#else
    LIBSSH2_EC_CURVE_NISTP521 = MBEDTLS_ECP_DP_NONE,
#endif
} libssh2_curve_type;

# define _libssh2_ec_key mbedtls_ecp_keypair
#else
# define _libssh2_ec_key void
#endif /* LIBSSH2_ECDSA */


/*******************************************************************/
/*
 * mbedTLS backend: ECDSA functions
 */

#if LIBSSH2_ECDSA

#define libssh2_ecdsa_ctx mbedtls_ecdsa_context

#define _libssh2_ecdsa_create_key(session, privkey, pubkey_octal, \
                                  pubkey_octal_len, curve) \
    _libssh2_mbedtls_ecdsa_create_key(session, privkey, pubkey_octal, \
                                      pubkey_octal_len, curve)

#define _libssh2_ecdsa_curve_name_with_octal_new(ctx, k, k_len, curve) \
    _libssh2_mbedtls_ecdsa_curve_name_with_octal_new(ctx, k, k_len, curve)

#define _libssh2_ecdh_gen_k(k, privkey, server_pubkey, server_pubkey_len) \
    _libssh2_mbedtls_ecdh_gen_k(k, privkey, server_pubkey, server_pubkey_len)

#define _libssh2_ecdsa_verify(ctx, r, r_len, s, s_len, m, m_len) \
    _libssh2_mbedtls_ecdsa_verify(ctx, r, r_len, s, s_len, m, m_len)

#define _libssh2_ecdsa_new_private(ctx, session, filename, passphrase) \
    _libssh2_mbedtls_ecdsa_new_private(ctx, session, filename, passphrase)

#define _libssh2_ecdsa_new_private_frommemory(ctx, session, filedata, \
                                              filedata_len, passphrase) \
    _libssh2_mbedtls_ecdsa_new_private_frommemory(ctx, session, filedata, \
                                                  filedata_len, passphrase)

#define _libssh2_ecdsa_sign(session, ctx, hash, hash_len, sign, sign_len) \
    _libssh2_mbedtls_ecdsa_sign(session, ctx, hash, hash_len, sign, sign_len)

#define _libssh2_ecdsa_get_curve_type(ctx) \
    _libssh2_mbedtls_ecdsa_get_curve_type(ctx)

#define _libssh2_ecdsa_free(ctx) \
    _libssh2_mbedtls_ecdsa_free(ctx)

#endif /* LIBSSH2_ECDSA */


/*******************************************************************/
/*
 * mbedTLS backend: Key functions
 */

#define _libssh2_pub_priv_keyfile(s, m, m_len, p, p_len, pk, pw) \
    _libssh2_mbedtls_pub_priv_keyfile(s, m, m_len, p, p_len, pk, pw)
#define _libssh2_pub_priv_keyfilememory(s, m, m_len, p, p_len, \
                                        pk, pk_len, pw) \
    _libssh2_mbedtls_pub_priv_keyfilememory(s, m, m_len, p, p_len, \
                                            pk, pk_len, pw)
#define _libssh2_sk_pub_keyfilememory(s, m, m_len, p, p_len, alg, app, \
                                      f, kh, kh_len, pk, pk_len, pw) \
    _libssh2_mbedtls_sk_pub_keyfilememory(s, m, m_len, p, p_len, alg, app, \
                                          f, kh, kh_len, pk, pk_len, pw)


/*******************************************************************/
/*
 * mbedTLS backend: Cipher Context structure
 */

#define _libssh2_cipher_ctx         mbedtls_cipher_context_t

#define _libssh2_cipher_type(algo)  mbedtls_cipher_type_t algo

#define _libssh2_cipher_aes256ctr MBEDTLS_CIPHER_AES_256_CTR
#define _libssh2_cipher_aes192ctr MBEDTLS_CIPHER_AES_192_CTR
#define _libssh2_cipher_aes128ctr MBEDTLS_CIPHER_AES_128_CTR
#define _libssh2_cipher_aes256    MBEDTLS_CIPHER_AES_256_CBC
#define _libssh2_cipher_aes192    MBEDTLS_CIPHER_AES_192_CBC
#define _libssh2_cipher_aes128    MBEDTLS_CIPHER_AES_128_CBC
#ifdef MBEDTLS_CIPHER_BLOWFISH_CBC
#define _libssh2_cipher_blowfish  MBEDTLS_CIPHER_BLOWFISH_CBC
#endif
#ifdef MBEDTLS_CIPHER_ARC4_128
#define _libssh2_cipher_arcfour   MBEDTLS_CIPHER_ARC4_128
#endif
#define _libssh2_cipher_3des      MBEDTLS_CIPHER_DES_EDE3_CBC
#define _libssh2_cipher_chacha20  MBEDTLS_CIPHER_CHACHA20_POLY1305


/*******************************************************************/
/*
 * mbedTLS backend: Cipher functions
 */

#define _libssh2_cipher_init(ctx, type, iv, secret, encrypt) \
    _libssh2_mbedtls_cipher_init(ctx, type, iv, secret, encrypt)
#define _libssh2_cipher_crypt(ctx, type, encrypt, block, blocklen, fl) \
    _libssh2_mbedtls_cipher_crypt(ctx, type, encrypt, block, blocklen, fl)
#define _libssh2_cipher_dtor(ctx) \
    _libssh2_mbedtls_cipher_dtor(ctx)


/*******************************************************************/
/*
 * mbedTLS backend: BigNumber Support
 */

#define _libssh2_bn_ctx int /* not used */
#define _libssh2_bn_ctx_new() 0 /* not used */
#define _libssh2_bn_ctx_free(bnctx) ((void)0) /* not used */

#define _libssh2_bn mbedtls_mpi

#define _libssh2_bn_init() \
    _libssh2_mbedtls_bignum_init()
#define _libssh2_bn_init_from_bin() \
    _libssh2_mbedtls_bignum_init()
#define _libssh2_bn_set_word(bn, word) \
    mbedtls_mpi_lset(bn, word)
#define _libssh2_bn_from_bin(bn, len, bin) \
    mbedtls_mpi_read_binary(bn, bin, len)
#define _libssh2_bn_to_bin(bn, bin) \
    mbedtls_mpi_write_binary(bn, bin, mbedtls_mpi_size(bn))
#define _libssh2_bn_bytes(bn) \
    mbedtls_mpi_size(bn)
#define _libssh2_bn_bits(bn) \
    mbedtls_mpi_bitlen(bn)
#define _libssh2_bn_free(bn) \
    _libssh2_mbedtls_bignum_free(bn)


/*******************************************************************/
/*
 * mbedTLS backend: Diffie-Hellman support.
 */

/* Default generate and safe prime sizes for
   diffie-hellman-group-exchange-sha1 */
#define LIBSSH2_DH_GEX_MINGROUP     2048
#define LIBSSH2_DH_GEX_OPTGROUP     4096
#define LIBSSH2_DH_GEX_MAXGROUP     8192

#define LIBSSH2_DH_MAX_MODULUS_BITS 16384

#define _libssh2_dh_ctx mbedtls_mpi *
#define libssh2_dh_init(dhctx) _libssh2_dh_init(dhctx)
#define libssh2_dh_key_pair(dhctx, public, g, p, group_order, bnctx) \
    _libssh2_dh_key_pair(dhctx, public, g, p, group_order)
#define libssh2_dh_secret(dhctx, secret, f, p, bnctx) \
    _libssh2_dh_secret(dhctx, secret, f, p)
#define libssh2_dh_dtor(dhctx) _libssh2_dh_dtor(dhctx)


/*******************************************************************/
/*
 * mbedTLS backend: forward declarations
 */

void
_libssh2_mbedtls_init(void);

void
_libssh2_mbedtls_free(void);

int
_libssh2_mbedtls_random(unsigned char *buf, size_t len);

void
_libssh2_mbedtls_cipher_dtor(_libssh2_cipher_ctx *ctx);

int
_libssh2_mbedtls_hash_init(mbedtls_md_context_t *ctx,
                           mbedtls_md_type_t mdtype,
                           const unsigned char *key, size_t keylen);

int
_libssh2_mbedtls_hash_final(mbedtls_md_context_t *ctx, unsigned char *hash);
int
_libssh2_mbedtls_hash(const unsigned char *data, size_t datalen,
                      mbedtls_md_type_t mdtype, unsigned char *hash);

_libssh2_bn *
_libssh2_mbedtls_bignum_init(void);

void
_libssh2_mbedtls_bignum_free(_libssh2_bn *bn);

void
_libssh2_mbedtls_rsa_free(libssh2_rsa_ctx *rsa);

#if LIBSSH2_ECDSA
libssh2_curve_type
_libssh2_mbedtls_ecdsa_key_get_curve_type(libssh2_ecdsa_ctx *ctx);
int
_libssh2_mbedtls_ecdsa_curve_type_from_name(const char *name,
                                            libssh2_curve_type *type);
void
_libssh2_mbedtls_ecdsa_free(libssh2_ecdsa_ctx *ctx);
#endif /* LIBSSH2_ECDSA */

extern void
_libssh2_init_aes_ctr(void);
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

#endif /* LIBSSH2_MBEDTLS_H */
