#ifndef LIBSSH2_CRYPTO_H
#define LIBSSH2_CRYPTO_H
/* Copyright (C) Simon Josefsson
 * Copyright (C) The Written Word, Inc.
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

#if defined(LIBSSH2_OPENSSL) || defined(LIBSSH2_WOLFSSL)
#include "openssl.h"
#elif defined(LIBSSH2_LIBGCRYPT)
#include "libgcrypt.h"
#elif defined(LIBSSH2_MBEDTLS)
#include "mbedtls.h"
#elif defined(LIBSSH2_OS400QC3)
#include "os400qc3.h"
#elif defined(LIBSSH2_WINCNG)
#include "wincng.h"
#else
#error "no cryptography backend selected"
#endif

/* return: success = 1, error = 0 */
int _libssh2_hmac_ctx_init(libssh2_hmac_ctx *ctx);
#if LIBSSH2_MD5
int _libssh2_hmac_md5_init(libssh2_hmac_ctx *ctx,
                           void *key, size_t keylen);
#endif
#if LIBSSH2_HMAC_RIPEMD
int _libssh2_hmac_ripemd160_init(libssh2_hmac_ctx *ctx,
                                 void *key, size_t keylen);
#endif
int _libssh2_hmac_sha1_init(libssh2_hmac_ctx *ctx,
                            void *key, size_t keylen);
int _libssh2_hmac_sha256_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen);
int _libssh2_hmac_sha512_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen);
int _libssh2_hmac_update(libssh2_hmac_ctx *ctx,
                         const void *data, size_t datalen);
int _libssh2_hmac_final(libssh2_hmac_ctx *ctx, void *data);
void _libssh2_hmac_cleanup(libssh2_hmac_ctx *ctx);

#define LIBSSH2_ED25519_KEY_LEN 32
#define LIBSSH2_ED25519_PRIVATE_KEY_LEN 64
#define LIBSSH2_ED25519_SIG_LEN 64

#if LIBSSH2_RSA
int _libssh2_rsa_new(libssh2_rsa_ctx ** rsa,
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
                     const unsigned char *coeffdata, unsigned long coefflen);
int _libssh2_rsa_new_private(libssh2_rsa_ctx ** rsa,
                             LIBSSH2_SESSION * session,
                             const char *filename,
                             unsigned const char *passphrase);
#if LIBSSH2_RSA_SHA1
int _libssh2_rsa_sha1_sign(LIBSSH2_SESSION * session,
                           libssh2_rsa_ctx * rsactx,
                           const unsigned char *hash,
                           size_t hash_len,
                           unsigned char **signature,
                           size_t *signature_len);
int _libssh2_rsa_sha1_verify(libssh2_rsa_ctx * rsa,
                             const unsigned char *sig,
                             size_t sig_len,
                             const unsigned char *m, size_t m_len);
#endif
#if LIBSSH2_RSA_SHA2
int _libssh2_rsa_sha2_sign(LIBSSH2_SESSION * session,
                           libssh2_rsa_ctx * rsactx,
                           const unsigned char *hash,
                           size_t hash_len,
                           unsigned char **signature,
                           size_t *signature_len);
int _libssh2_rsa_sha2_verify(libssh2_rsa_ctx * rsa,
                             size_t hash_len,
                             const unsigned char *sig,
                             size_t sig_len,
                             const unsigned char *m, size_t m_len);
#endif
int _libssh2_rsa_new_private_frommemory(libssh2_rsa_ctx ** rsa,
                                        LIBSSH2_SESSION * session,
                                        const char *filedata,
                                        size_t filedata_len,
                                        unsigned const char *passphrase);
#endif

#if LIBSSH2_DSA
int _libssh2_dsa_new(libssh2_dsa_ctx ** dsa,
                     const unsigned char *pdata,
                     unsigned long plen,
                     const unsigned char *qdata,
                     unsigned long qlen,
                     const unsigned char *gdata,
                     unsigned long glen,
                     const unsigned char *ydata,
                     unsigned long ylen,
                     const unsigned char *x, unsigned long x_len);
int _libssh2_dsa_new_private(libssh2_dsa_ctx ** dsa,
                             LIBSSH2_SESSION * session,
                             const char *filename,
                             unsigned const char *passphrase);
int _libssh2_dsa_sha1_verify(libssh2_dsa_ctx * dsactx,
                             const unsigned char *sig,
                             const unsigned char *m, size_t m_len);
int _libssh2_dsa_sha1_sign(libssh2_dsa_ctx * dsactx,
                           const unsigned char *hash,
                           size_t hash_len, unsigned char *sig);
int _libssh2_dsa_new_private_frommemory(libssh2_dsa_ctx ** dsa,
                                        LIBSSH2_SESSION * session,
                                        const char *filedata,
                                        size_t filedata_len,
                                        unsigned const char *passphrase);
#endif

#if LIBSSH2_ECDSA
int
_libssh2_ecdsa_curve_name_with_octal_new(libssh2_ecdsa_ctx ** ecdsactx,
                                         const unsigned char *k,
                                         size_t k_len,
                                         libssh2_curve_type type);

int
_libssh2_ecdsa_new_private(libssh2_ecdsa_ctx ** ec_ctx,
                           LIBSSH2_SESSION * session,
                           const char *filename,
                           unsigned const char *passphrase);

int
_libssh2_ecdsa_new_private_sk(libssh2_ecdsa_ctx ** ec_ctx,
                              unsigned char *flags,
                              const char **application,
                              const unsigned char **key_handle,
                              size_t *handle_len,
                              LIBSSH2_SESSION * session,
                              const char *filename,
                              unsigned const char *passphrase);

int
_libssh2_ecdsa_verify(libssh2_ecdsa_ctx * ctx,
                      const unsigned char *r, size_t r_len,
                      const unsigned char *s, size_t s_len,
                      const unsigned char *m, size_t m_len);

int
_libssh2_ecdsa_create_key(LIBSSH2_SESSION *session,
                          _libssh2_ec_key **out_private_key,
                          unsigned char **out_public_key_octal,
                          size_t *out_public_key_octal_len,
                          libssh2_curve_type curve_type);

int
_libssh2_ecdh_gen_k(_libssh2_bn **k, _libssh2_ec_key *private_key,
                    const unsigned char *server_public_key,
                    size_t server_public_key_len);

int
_libssh2_ecdsa_sign(LIBSSH2_SESSION *session, libssh2_ecdsa_ctx *ec_ctx,
                    const unsigned char *hash, size_t hash_len,
                    unsigned char **signature, size_t *signature_len);

int _libssh2_ecdsa_new_private_frommemory(libssh2_ecdsa_ctx ** ec_ctx,
                                          LIBSSH2_SESSION * session,
                                          const char *filedata,
                                          size_t filedata_len,
                                          unsigned const char *passphrase);

int _libssh2_ecdsa_new_private_frommemory_sk(libssh2_ecdsa_ctx ** ec_ctx,
                                             unsigned char *flags,
                                             const char **application,
                                             const unsigned char **key_handle,
                                             size_t *handle_len,
                                             LIBSSH2_SESSION * session,
                                             const char *filedata,
                                             size_t filedata_len,
                                             unsigned const char *passphrase);

libssh2_curve_type
_libssh2_ecdsa_get_curve_type(libssh2_ecdsa_ctx *ec_ctx);

int
_libssh2_ecdsa_curve_type_from_name(const char *name,
                                    libssh2_curve_type *out_type);

#endif /* LIBSSH2_ECDSA */

#if LIBSSH2_ED25519

int
_libssh2_curve25519_new(LIBSSH2_SESSION *session, uint8_t **out_public_key,
                        uint8_t **out_private_key);

int
_libssh2_curve25519_gen_k(_libssh2_bn **k,
                          uint8_t private_key[LIBSSH2_ED25519_KEY_LEN],
                          uint8_t server_public_key[LIBSSH2_ED25519_KEY_LEN]);

int
_libssh2_ed25519_verify(libssh2_ed25519_ctx *ctx, const uint8_t *s,
                        size_t s_len, const uint8_t *m, size_t m_len);

int
_libssh2_ed25519_new_private(libssh2_ed25519_ctx **ed_ctx,
                            LIBSSH2_SESSION *session,
                            const char *filename, const uint8_t *passphrase);

int
_libssh2_ed25519_new_private_sk(libssh2_ed25519_ctx **ed_ctx,
                                unsigned char *flags,
                                const char **application,
                                const unsigned char **key_handle,
                                size_t *handle_len,
                                LIBSSH2_SESSION *session,
                                const char *filename,
                                const uint8_t *passphrase);

int
_libssh2_ed25519_new_public(libssh2_ed25519_ctx **ed_ctx,
                            LIBSSH2_SESSION *session,
                            const unsigned char *raw_pub_key,
                            const size_t key_len);

int
_libssh2_ed25519_sign(libssh2_ed25519_ctx *ctx, LIBSSH2_SESSION *session,
                      uint8_t **out_sig, size_t *out_sig_len,
                      const uint8_t *message, size_t message_len);

int
_libssh2_ed25519_new_private_frommemory(libssh2_ed25519_ctx **ed_ctx,
                                        LIBSSH2_SESSION *session,
                                        const char *filedata,
                                        size_t filedata_len,
                                        unsigned const char *passphrase);

int
_libssh2_ed25519_new_private_frommemory_sk(libssh2_ed25519_ctx **ed_ctx,
                                           unsigned char *flags,
                                           const char **application,
                                           const unsigned char **key_handle,
                                           size_t *handle_len,
                                           LIBSSH2_SESSION *session,
                                           const char *filedata,
                                           size_t filedata_len,
                                           unsigned const char *passphrase);

#endif /* LIBSSH2_ED25519 */


int _libssh2_cipher_init(_libssh2_cipher_ctx * h,
                         _libssh2_cipher_type(algo),
                         unsigned char *iv,
                         unsigned char *secret, int encrypt);

int _libssh2_cipher_crypt(_libssh2_cipher_ctx * ctx,
                          _libssh2_cipher_type(algo),
                          int encrypt, unsigned char *block, size_t blocksize,
                          int firstlast);

int _libssh2_pub_priv_keyfile(LIBSSH2_SESSION *session,
                              unsigned char **method,
                              size_t *method_len,
                              unsigned char **pubkeydata,
                              size_t *pubkeydata_len,
                              const char *privatekey,
                              const char *passphrase);

int _libssh2_pub_priv_keyfilememory(LIBSSH2_SESSION *session,
                                    unsigned char **method,
                                    size_t *method_len,
                                    unsigned char **pubkeydata,
                                    size_t *pubkeydata_len,
                                    const char *privatekeydata,
                                    size_t privatekeydata_len,
                                    const char *passphrase);


int _libssh2_sk_pub_keyfilememory(LIBSSH2_SESSION *session,
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
                                  const char *passphrase);

/**
 * @function _libssh2_supported_key_sign_algorithms
 * @abstract Returns supported algorithms used for upgrading public
 * key signing RFC 8332
 * @discussion Based on the incoming key_method value, this function
 * will return supported algorithms that can upgrade the key method
 * @related _libssh2_key_sign_algorithm()
 * @param key_method current key method, usually the default key sig method
 * @param key_method_len length of the key method buffer
 * @result comma separated list of supported upgrade options per RFC 8332, if
 * there is no upgrade option return NULL
 */

const char *
_libssh2_supported_key_sign_algorithms(LIBSSH2_SESSION *session,
                                       unsigned char *key_method,
                                       size_t key_method_len);

#endif /* LIBSSH2_CRYPTO_H */
