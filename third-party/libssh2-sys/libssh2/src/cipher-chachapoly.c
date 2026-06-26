/*
 * Copyright (c) 2013 Damien Miller <djm@mindrot.org>
 *
 * Adapted by Will Cosgrove <will@panic.com> for libssh2
 *
 * Permission to use, copy, modify, and distribute this software for any
 * purpose with or without fee is hereby granted, provided that the above
 * copyright notice and this permission notice appear in all copies.
 *
 * THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
 * WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
 * ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
 * WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
 * ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
 * OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
 *
 * SPDX-License-Identifier: BSD-2-Clause
 */

/* $OpenBSD: cipher-chachapoly.c,v 1.8 2016/08/03 05:41:57 djm Exp $ */

#include "libssh2_priv.h"
#include "misc.h"
#include "cipher-chachapoly.h"

int
chachapoly_timingsafe_bcmp(const void *b1, const void *b2, size_t n);

int
chachapoly_init(struct chachapoly_ctx *ctx, const u_char *key, u_int keylen)
{
    if(keylen != (32 + 32)) /* 2 x 256 bit keys */
        return LIBSSH2_ERROR_INVAL;
    chacha_keysetup(&ctx->main_ctx, key, 256);
    chacha_keysetup(&ctx->header_ctx, key + 32, 256);
    return 0;
}

/*
 * chachapoly_crypt() operates as following:
 * En/decrypt with header key 'aadlen' bytes from 'src', storing result
 * to 'dest'. The ciphertext here is treated as additional authenticated
 * data for MAC calculation.
 * En/decrypt 'len' bytes at offset 'aadlen' from 'src' to 'dest'. Use
 * POLY1305_TAGLEN bytes at offset 'len'+'aadlen' as the authentication
 * tag. This tag is written on encryption and verified on decryption.
 */
int
chachapoly_crypt(struct chachapoly_ctx *ctx, u_int seqnr, u_char *dest,
                 const u_char *src, u_int len, u_int aadlen, int do_encrypt)
{
    u_char seqbuf[8];
    const u_char one[8] = { 1, 0, 0, 0, 0, 0, 0, 0 }; /* NB little-endian */
    u_char expected_tag[POLY1305_TAGLEN], poly_key[POLY1305_KEYLEN];
    int r = LIBSSH2_ERROR_INVAL;
    unsigned char *ptr = NULL;

    /*
     * Run ChaCha20 once to generate the Poly1305 key. The IV is the
     * packet sequence number.
     */
    memset(poly_key, 0, sizeof(poly_key));
    ptr = &seqbuf[0];
    _libssh2_store_u64(&ptr, seqnr);
    chacha_ivsetup(&ctx->main_ctx, seqbuf, NULL);
    chacha_encrypt_bytes(&ctx->main_ctx,
                         poly_key, poly_key, sizeof(poly_key));

    /* If decrypting, check tag before anything else */
    if(!do_encrypt) {
        const u_char *tag = src + aadlen + len;

        poly1305_auth(expected_tag, src, aadlen + len, poly_key);
        if(chachapoly_timingsafe_bcmp(expected_tag, tag, POLY1305_TAGLEN)
           != 0) {
            r = LIBSSH2_ERROR_DECRYPT;
            goto out;
        }
    }

    /* Crypt additional data */
    if(aadlen) {
        chacha_ivsetup(&ctx->header_ctx, seqbuf, NULL);
        chacha_encrypt_bytes(&ctx->header_ctx, src, dest, aadlen);
    }

    /* Set Chacha's block counter to 1 */
    chacha_ivsetup(&ctx->main_ctx, seqbuf, one);
    chacha_encrypt_bytes(&ctx->main_ctx, src + aadlen,
                         dest + aadlen, len);

    /* If encrypting, calculate and append tag */
    if(do_encrypt) {
        poly1305_auth(dest + aadlen + len, dest, aadlen + len,
                      poly_key);
    }
    r = 0;
out:
    memset(expected_tag, 0, sizeof(expected_tag));
    memset(seqbuf, 0, sizeof(seqbuf));
    memset(poly_key, 0, sizeof(poly_key));
    return r;
}

/* Decrypt and extract the encrypted packet length */
int
chachapoly_get_length(struct chachapoly_ctx *ctx, unsigned int *plenp,
                      unsigned int seqnr, const unsigned char *cp,
                      unsigned int len)
{
    u_char buf[4], seqbuf[8];
    unsigned char *ptr = NULL;

    if(len < 4)
        return -1;
    ptr = &seqbuf[0];
    _libssh2_store_u64(&ptr, seqnr);
    chacha_ivsetup(&ctx->header_ctx, seqbuf, NULL);
    chacha_encrypt_bytes(&ctx->header_ctx, cp, buf, 4);
    *plenp = _libssh2_ntohu32(buf);
    return 0;
}

int
chachapoly_timingsafe_bcmp(const void *b1, const void *b2, size_t n)
{
    const unsigned char *p1 = b1, *p2 = b2;
    int ret = 0;

    for(; n > 0; n--)
        ret |= *p1++ ^ *p2++;
    return (ret != 0);
}
