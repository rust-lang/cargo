/* Copyright (C) Sara Golemon <sarag@libssh2.org>
 * Copyright (C) Daniel Stenberg <daniel@haxx.se>
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

#include "transport.h"
#include "comp.h"
#include "mac.h"

#include <assert.h>

/* define SHA1_DIGEST_LENGTH for the macro below */
#ifndef SHA1_DIGEST_LENGTH
#define SHA1_DIGEST_LENGTH SHA_DIGEST_LENGTH
#endif

/* TODO: Switch this to an inline and handle alloc() failures */
/* Helper macro called from
   kex_method_diffie_hellman_group1_sha1_key_exchange */

#if LIBSSH2_ECDSA
#define LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(value, reqlen, version)        \
    do {                                                                    \
        if(type == LIBSSH2_EC_CURVE_NISTP256) {                             \
            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, value, reqlen, version); \
        }                                                                   \
        else if(type == LIBSSH2_EC_CURVE_NISTP384) {                        \
            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(384, value, reqlen, version); \
        }                                                                   \
        else if(type == LIBSSH2_EC_CURVE_NISTP521) {                        \
            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(512, value, reqlen, version); \
        }                                                                   \
    } while(0)
#endif

#define LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(digest_type, value,               \
                                          reqlen, version)                  \
do {                                                                        \
    libssh2_sha##digest_type##_ctx hash;                                    \
    size_t len = 0;                                                         \
    if(!(value)) {                                                          \
        value = LIBSSH2_ALLOC(session,                                      \
                              reqlen + SHA##digest_type##_DIGEST_LENGTH);   \
    }                                                                       \
    if(value)                                                               \
        while(len < (size_t)reqlen) {                                       \
            if(!libssh2_sha##digest_type##_init(&hash) ||                   \
               !libssh2_sha##digest_type##_update(hash,                     \
                                       exchange_state->k_value,             \
                                       exchange_state->k_value_len) ||      \
               !libssh2_sha##digest_type##_update(hash,                     \
                                       exchange_state->h_sig_comp,          \
                                       SHA##digest_type##_DIGEST_LENGTH)) { \
                LIBSSH2_FREE(session, value);                               \
                value = NULL;                                               \
                break;                                                      \
            }                                                               \
            if(len > 0) {                                                   \
                if(!libssh2_sha##digest_type##_update(hash, value, len)) {  \
                    LIBSSH2_FREE(session, value);                           \
                    value = NULL;                                           \
                    break;                                                  \
                }                                                           \
            }                                                               \
            else {                                                          \
                if(!libssh2_sha##digest_type##_update(hash,                 \
                                                      (version), 1) ||      \
                   !libssh2_sha##digest_type##_update(hash,                 \
                                                session->session_id,        \
                                                session->session_id_len)) { \
                    LIBSSH2_FREE(session, value);                           \
                    value = NULL;                                           \
                    break;                                                  \
                }                                                           \
            }                                                               \
            if(!libssh2_sha##digest_type##_final(hash, (value) + len)) {    \
                LIBSSH2_FREE(session, value);                               \
                value = NULL;                                               \
                break;                                                      \
            }                                                               \
            len += SHA##digest_type##_DIGEST_LENGTH;                        \
        }                                                                   \
} while(0)

/*!
 * @note The following are wrapper functions used by diffie_hellman_sha_algo().
 * TODO: Switch backend SHA macros to functions to allow function pointers
 * @discussion Ideally these would be function pointers but the backend macros
 * don't allow it so we have to wrap them up in helper functions
 */

static int _libssh2_sha_algo_ctx_init(int sha_algo, void *ctx)
{
    if(sha_algo == 512) {
        return libssh2_sha512_init((libssh2_sha512_ctx*)ctx);
    }
    else if(sha_algo == 384) {
        return libssh2_sha384_init((libssh2_sha384_ctx*)ctx);
    }
    else if(sha_algo == 256) {
        return libssh2_sha256_init((libssh2_sha256_ctx*)ctx);
    }
    else if(sha_algo == 1) {
        return libssh2_sha1_init((libssh2_sha1_ctx*)ctx);
    }
    else {
#ifdef LIBSSH2DEBUG
        assert(0);
#endif
    }
    return 0;
}

static int _libssh2_sha_algo_ctx_update(int sha_algo, void *ctx,
                                        void *data, size_t len)
{
    if(sha_algo == 512) {
        libssh2_sha512_ctx *_ctx = (libssh2_sha512_ctx*)ctx;
        return libssh2_sha512_update(*_ctx, data, len);
    }
    else if(sha_algo == 384) {
        libssh2_sha384_ctx *_ctx = (libssh2_sha384_ctx*)ctx;
        return libssh2_sha384_update(*_ctx, data, len);
    }
    else if(sha_algo == 256) {
        libssh2_sha256_ctx *_ctx = (libssh2_sha256_ctx*)ctx;
        return libssh2_sha256_update(*_ctx, data, len);
    }
    else if(sha_algo == 1) {
        libssh2_sha1_ctx *_ctx = (libssh2_sha1_ctx*)ctx;
        return libssh2_sha1_update(*_ctx, data, len);
    }
    else {
#ifdef LIBSSH2DEBUG
        assert(0);
#endif
    }
    return 0;
}

static int _libssh2_sha_algo_ctx_final(int sha_algo, void *ctx,
                                       void *hash)
{
    if(sha_algo == 512) {
        libssh2_sha512_ctx *_ctx = (libssh2_sha512_ctx*)ctx;
        return libssh2_sha512_final(*_ctx, hash);
    }
    else if(sha_algo == 384) {
        libssh2_sha384_ctx *_ctx = (libssh2_sha384_ctx*)ctx;
        return libssh2_sha384_final(*_ctx, hash);
    }
    else if(sha_algo == 256) {
        libssh2_sha256_ctx *_ctx = (libssh2_sha256_ctx*)ctx;
        return libssh2_sha256_final(*_ctx, hash);
    }
    else if(sha_algo == 1) {
        libssh2_sha1_ctx *_ctx = (libssh2_sha1_ctx*)ctx;
        return libssh2_sha1_final(*_ctx, hash);
    }
    else {
#ifdef LIBSSH2DEBUG
        assert(0);
#endif
    }
    return 0;
}

static void _libssh2_sha_algo_value_hash(int sha_algo,
                                         LIBSSH2_SESSION *session,
                                         kmdhgGPshakex_state_t *exchange_state,
                                         unsigned char **data, size_t data_len,
                                         const unsigned char *version)
{
    if(sha_algo == 512) {
        LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(512, *data, data_len, version);
    }
    else if(sha_algo == 384) {
        LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(384, *data, data_len, version);
    }
    else if(sha_algo == 256) {
        LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, *data, data_len, version);
    }
    else if(sha_algo == 1) {
        LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(1, *data, data_len, version);
    }
    else {
#ifdef LIBSSH2DEBUG
        assert(0);
#endif
    }
}


static void
diffie_hellman_state_cleanup(LIBSSH2_SESSION * session,
                             kmdhgGPshakex_state_t *exchange_state)
{
    libssh2_dh_dtor(&exchange_state->x);
    _libssh2_bn_free(exchange_state->e);
    exchange_state->e = NULL;
    _libssh2_bn_free(exchange_state->f);
    exchange_state->f = NULL;
    _libssh2_bn_free(exchange_state->k);
    exchange_state->k = NULL;
    _libssh2_bn_ctx_free(exchange_state->ctx);
    exchange_state->ctx = NULL;

    if(exchange_state->e_packet) {
        LIBSSH2_FREE(session, exchange_state->e_packet);
        exchange_state->e_packet = NULL;
    }

    if(exchange_state->s_packet) {
        LIBSSH2_FREE(session, exchange_state->s_packet);
        exchange_state->s_packet = NULL;
    }

    if(exchange_state->k_value) {
        LIBSSH2_FREE(session, exchange_state->k_value);
        exchange_state->k_value = NULL;
    }

    exchange_state->state = libssh2_NB_state_idle;
}

static void
kex_diffie_hellman_cleanup(LIBSSH2_SESSION * session,
                           key_exchange_state_low_t * key_state) {
    if(key_state->state != libssh2_NB_state_idle) {
        _libssh2_bn_free(key_state->p);
        key_state->p = NULL;
        _libssh2_bn_free(key_state->g);
        key_state->g = NULL;

        if(key_state->data) {
            LIBSSH2_FREE(session, key_state->data);
            key_state->data = NULL;
        }
        key_state->state = libssh2_NB_state_idle;
    }

    if(key_state->exchange_state.state != libssh2_NB_state_idle) {
        diffie_hellman_state_cleanup(session, &key_state->exchange_state);
    }
}


/*!
 * @function diffie_hellman_sha_algo
 * @abstract Diffie Hellman Key Exchange, Group Agnostic,
 * SHA Algorithm Agnostic
 * @result 0 on success, error code on failure
 */
static int diffie_hellman_sha_algo(LIBSSH2_SESSION *session,
                                   _libssh2_bn *g,
                                   _libssh2_bn *p,
                                   int group_order,
                                   int sha_algo_value,
                                   void *exchange_hash_ctx,
                                   unsigned char packet_type_init,
                                   unsigned char packet_type_reply,
                                   unsigned char *midhash,
                                   size_t midhash_len,
                                   kmdhgGPshakex_state_t *exchange_state)
{
    int ret = 0;
    int rc;

    int digest_len = 0;

    if(sha_algo_value == 512)
        digest_len = SHA512_DIGEST_LENGTH;
    else if(sha_algo_value == 384)
        digest_len = SHA384_DIGEST_LENGTH;
    else if(sha_algo_value == 256)
        digest_len = SHA256_DIGEST_LENGTH;
    else if(sha_algo_value == 1)
        digest_len = SHA1_DIGEST_LENGTH;
    else {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "sha algo value is unimplemented");
        goto clean_exit;
    }

    if(exchange_state->state == libssh2_NB_state_idle) {
        /* Setup initial values */
        exchange_state->e_packet = NULL;
        exchange_state->s_packet = NULL;
        exchange_state->k_value = NULL;
        exchange_state->ctx = _libssh2_bn_ctx_new();
        libssh2_dh_init(&exchange_state->x);
        exchange_state->e = _libssh2_bn_init(); /* g^x mod p */
        exchange_state->f = _libssh2_bn_init_from_bin(); /* g^(Random from
                                                            server) mod p */
        exchange_state->k = _libssh2_bn_init(); /* The shared secret: f^x mod
                                                   p */

        /* Zero the whole thing out */
        memset(&exchange_state->req_state, 0, sizeof(packet_require_state_t));

        /* Generate x and e */
        if(_libssh2_bn_bits(p) > LIBSSH2_DH_MAX_MODULUS_BITS) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_INVAL,
                                 "dh modulus value is too large");
            goto clean_exit;
        }

        rc = libssh2_dh_key_pair(&exchange_state->x, exchange_state->e, g, p,
                                 group_order, exchange_state->ctx);
        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_KEX_FAILURE,
                                 "dh key pair generation failed");
            goto clean_exit;
        }

        /* Send KEX init */
        /* packet_type(1) + String Length(4) + leading 0(1) */
        exchange_state->e_packet_len =
            _libssh2_bn_bytes(exchange_state->e) + 6;
        if(_libssh2_bn_bits(exchange_state->e) % 8) {
            /* Leading 00 not needed */
            exchange_state->e_packet_len--;
        }

        exchange_state->e_packet =
            LIBSSH2_ALLOC(session, exchange_state->e_packet_len);
        if(!exchange_state->e_packet) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Out of memory error");
            goto clean_exit;
        }
        exchange_state->e_packet[0] = packet_type_init;
        _libssh2_htonu32(exchange_state->e_packet + 1,
                         (uint32_t)(exchange_state->e_packet_len - 5));
        if(_libssh2_bn_bits(exchange_state->e) % 8) {
            if(_libssh2_bn_to_bin(exchange_state->e,
                                  exchange_state->e_packet + 5)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->e");
                goto clean_exit;
            }
        }
        else {
            exchange_state->e_packet[5] = 0;
            if(_libssh2_bn_to_bin(exchange_state->e,
                                  exchange_state->e_packet + 6)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->e");
                goto clean_exit;
            }
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX, "Sending KEX packet %u",
                       (unsigned int) packet_type_init));
        exchange_state->state = libssh2_NB_state_created;
    }

    if(exchange_state->state == libssh2_NB_state_created) {
        rc = _libssh2_transport_send(session, exchange_state->e_packet,
                                     exchange_state->e_packet_len,
                                     NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send KEX init message");
            goto clean_exit;
        }
        exchange_state->state = libssh2_NB_state_sent;
    }

    if(exchange_state->state == libssh2_NB_state_sent) {
        if(session->burn_optimistic_kexinit) {
            /* The first KEX packet to come along will be the guess initially
             * sent by the server.  That guess turned out to be wrong so we
             * need to silently ignore it */
            int burn_type;

            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Waiting for badly guessed KEX packet "
                           "(to be ignored)"));
            burn_type =
                _libssh2_packet_burn(session, &exchange_state->burn_state);
            if(burn_type == LIBSSH2_ERROR_EAGAIN) {
                return burn_type;
            }
            else if(burn_type <= 0) {
                /* Failed to receive a packet */
                ret = burn_type;
                goto clean_exit;
            }
            session->burn_optimistic_kexinit = 0;

            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Burnt packet of type: %02x",
                           (unsigned int) burn_type));
        }

        exchange_state->state = libssh2_NB_state_sent1;
    }

    if(exchange_state->state == libssh2_NB_state_sent1) {
        /* Wait for KEX reply */
        struct string_buf buf;
        size_t host_key_len;
        int err;
        int hok;

        rc = _libssh2_packet_require(session, packet_type_reply,
                                     &exchange_state->s_packet,
                                     &exchange_state->s_packet_len, 0, NULL,
                                     0, &exchange_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_TIMEOUT,
                                 "Timed out waiting for KEX reply");
            goto clean_exit;
        }

        /* Parse KEXDH_REPLY */
        if(exchange_state->s_packet_len < 5) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected packet length DH-SHA");
            goto clean_exit;
        }

        buf.data = exchange_state->s_packet;
        buf.len = exchange_state->s_packet_len;
        buf.dataptr = buf.data;
        buf.dataptr++; /* advance past type */

        if(session->server_hostkey) {
            LIBSSH2_FREE(session, session->server_hostkey);
            session->server_hostkey = NULL;
            session->server_hostkey_len = 0;
        }

        if(_libssh2_copy_string(session, &buf, &(session->server_hostkey),
                                &host_key_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Could not copy host key");
            goto clean_exit;
        }

        session->server_hostkey_len = (uint32_t)host_key_len;

#if LIBSSH2_MD5
        {
            libssh2_md5_ctx fingerprint_ctx;

            if(libssh2_md5_init(&fingerprint_ctx) &&
               libssh2_md5_update(fingerprint_ctx, session->server_hostkey,
                                  session->server_hostkey_len) &&
               libssh2_md5_final(fingerprint_ctx,
                                 session->server_hostkey_md5)) {
                session->server_hostkey_md5_valid = TRUE;
            }
            else {
                session->server_hostkey_md5_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[50], *fprint = fingerprint;
            int i;
            for(i = 0; i < 16; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_md5[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's MD5 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */
#endif /* ! LIBSSH2_MD5 */

        {
            libssh2_sha1_ctx fingerprint_ctx;

            if(libssh2_sha1_init(&fingerprint_ctx) &&
               libssh2_sha1_update(fingerprint_ctx, session->server_hostkey,
                                   session->server_hostkey_len) &&
               libssh2_sha1_final(fingerprint_ctx,
                                  session->server_hostkey_sha1)) {
                session->server_hostkey_sha1_valid = TRUE;
            }
            else {
                session->server_hostkey_sha1_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[64], *fprint = fingerprint;
            int i;
            for(i = 0; i < 20; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_sha1[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's SHA1 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */

        {
            libssh2_sha256_ctx fingerprint_ctx;

            if(libssh2_sha256_init(&fingerprint_ctx) &&
               libssh2_sha256_update(fingerprint_ctx, session->server_hostkey,
                                     session->server_hostkey_len) &&
               libssh2_sha256_final(fingerprint_ctx,
                                    session->server_hostkey_sha256)) {
                session->server_hostkey_sha256_valid = TRUE;
            }
            else {
                session->server_hostkey_sha256_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char *base64Fingerprint = NULL;
            _libssh2_base64_encode(session,
                                   (const char *)
                                   session->server_hostkey_sha256,
                                   SHA256_DIGEST_LENGTH, &base64Fingerprint);
            if(base64Fingerprint) {
                _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                               "Server's SHA256 Fingerprint: %s",
                               base64Fingerprint));
                LIBSSH2_FREE(session, base64Fingerprint);
            }
        }
#endif /* LIBSSH2DEBUG */


        if(session->hostkey->init(session, session->server_hostkey,
                                  session->server_hostkey_len,
                                  &session->server_hostkey_abstract)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unable to initialize hostkey importer "
                                 "DH-SHA");
            goto clean_exit;
        }

        if(_libssh2_get_string(&buf, &(exchange_state->f_value),
                               &(exchange_state->f_value_len))) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unable to get DH-SHA f value");
            goto clean_exit;
        }

        if(_libssh2_bn_from_bin(exchange_state->f,
                                exchange_state->f_value_len,
                                exchange_state->f_value)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Invalid DH-SHA f value");
            goto clean_exit;
        }

        if(_libssh2_get_string(&buf, &(exchange_state->h_sig),
                               &(exchange_state->h_sig_len))) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unable to get DH-SHA h sig");
            goto clean_exit;
        }

        /* Compute the shared secret */
        libssh2_dh_secret(&exchange_state->x, exchange_state->k,
                          exchange_state->f, p, exchange_state->ctx);
        exchange_state->k_value_len = _libssh2_bn_bytes(exchange_state->k) + 5;
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            /* don't need leading 00 */
            exchange_state->k_value_len--;
        }
        exchange_state->k_value =
            LIBSSH2_ALLOC(session, exchange_state->k_value_len);
        if(!exchange_state->k_value) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate buffer for DH-SHA K");
            goto clean_exit;
        }
        _libssh2_htonu32(exchange_state->k_value,
                         (uint32_t)(exchange_state->k_value_len - 4));
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 4)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->k");
                goto clean_exit;
            }
        }
        else {
            exchange_state->k_value[4] = 0;
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 5)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->k");
                goto clean_exit;
            }
        }

        exchange_state->exchange_hash = (void *)&exchange_hash_ctx;
        if(!_libssh2_sha_algo_ctx_init(sha_algo_value, exchange_hash_ctx)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HASH_INIT,
                                 "Unable to initialize hash context");
            goto clean_exit;
        }
        hok = 1;
        if(session->local.banner) {
            _libssh2_htonu32(exchange_state->h_sig_comp,
                (uint32_t)(strlen((char *) session->local.banner) - 2));
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                                exchange_state->h_sig_comp, 4);
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                                session->local.banner,
                                   strlen((char *) session->local.banner) - 2);
        }
        else {
            _libssh2_htonu32(exchange_state->h_sig_comp,
                             sizeof(LIBSSH2_SSH_DEFAULT_BANNER) - 1);
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                                exchange_state->h_sig_comp, 4);
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                   (unsigned char *)LIBSSH2_SSH_DEFAULT_BANNER,
                                       sizeof(LIBSSH2_SSH_DEFAULT_BANNER) - 1);
        }

        _libssh2_htonu32(exchange_state->h_sig_comp,
                         (uint32_t)strlen((char *) session->remote.banner));
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->h_sig_comp, 4);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            session->remote.banner,
                                       strlen((char *)session->remote.banner));

        _libssh2_htonu32(exchange_state->h_sig_comp,
                         (uint32_t)session->local.kexinit_len);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->h_sig_comp, 4);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            session->local.kexinit,
                                            session->local.kexinit_len);

        _libssh2_htonu32(exchange_state->h_sig_comp,
                         (uint32_t)session->remote.kexinit_len);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->h_sig_comp, 4);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            session->remote.kexinit,
                                            session->remote.kexinit_len);

        _libssh2_htonu32(exchange_state->h_sig_comp,
                         session->server_hostkey_len);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->h_sig_comp, 4);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            session->server_hostkey,
                                            session->server_hostkey_len);

        if(packet_type_init == SSH_MSG_KEX_DH_GEX_INIT) {
            /* diffie-hellman-group-exchange hashes additional fields */
            _libssh2_htonu32(exchange_state->h_sig_comp,
                             LIBSSH2_DH_GEX_MINGROUP);
            _libssh2_htonu32(exchange_state->h_sig_comp + 4,
                             LIBSSH2_DH_GEX_OPTGROUP);
            _libssh2_htonu32(exchange_state->h_sig_comp + 8,
                             LIBSSH2_DH_GEX_MAXGROUP);
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                                exchange_state->h_sig_comp,
                                                12);
        }

        if(midhash) {
            hok &= _libssh2_sha_algo_ctx_update(sha_algo_value,
                                                exchange_hash_ctx,
                                                midhash, midhash_len);
        }

        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->e_packet + 1,
                                            exchange_state->e_packet_len - 1);

        _libssh2_htonu32(exchange_state->h_sig_comp,
                         (uint32_t)exchange_state->f_value_len);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->h_sig_comp, 4);
        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->f_value,
                                            exchange_state->f_value_len);

        hok &= _libssh2_sha_algo_ctx_update(sha_algo_value, exchange_hash_ctx,
                                            exchange_state->k_value,
                                            exchange_state->k_value_len);

        if(!hok ||
           !_libssh2_sha_algo_ctx_final(sha_algo_value, exchange_hash_ctx,
                                        exchange_state->h_sig_comp)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HASH_CALC,
                                 "kex: failed to calculate hash");
            goto clean_exit;
        }

        err = session->hostkey->sig_verify(session,
                                           exchange_state->h_sig,
                                           exchange_state->h_sig_len,
                                           exchange_state->h_sig_comp,
                                           digest_len,
                                           &session->server_hostkey_abstract);

        if(err) {
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Failed hostkey sig_verify(): %s: %d",
                           session->hostkey->name, err));
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_SIGN,
                                 "Unable to verify hostkey signature "
                                 "DH-SHA");
            goto clean_exit;
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Sending NEWKEYS message"));
        exchange_state->c = SSH_MSG_NEWKEYS;

        exchange_state->state = libssh2_NB_state_sent2;
    }

    if(exchange_state->state == libssh2_NB_state_sent2) {
        rc = _libssh2_transport_send(session, &exchange_state->c, 1, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send NEWKEYS message DH-SHA");
            goto clean_exit;
        }

        exchange_state->state = libssh2_NB_state_sent3;
    }

    if(exchange_state->state == libssh2_NB_state_sent3) {
        rc = _libssh2_packet_require(session, SSH_MSG_NEWKEYS,
                                     &exchange_state->tmp,
                                     &exchange_state->tmp_len, 0, NULL, 0,
                                     &exchange_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timed out waiting for NEWKEYS DH-SHA");
            goto clean_exit;
        }

        /* The first key exchange has been performed,
           switch to active crypt/comp/mac mode */
        session->state |= LIBSSH2_STATE_NEWKEYS;
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Received NEWKEYS message DH-SHA"));

        /* This will actually end up being just packet_type(1)
           for this packet type anyway */
        LIBSSH2_FREE(session, exchange_state->tmp);

        if(!session->session_id) {
            session->session_id = LIBSSH2_ALLOC(session, digest_len);
            if(!session->session_id) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                     "Unable to allocate buffer for "
                                     "SHA digest");
                goto clean_exit;
            }
            memcpy(session->session_id, exchange_state->h_sig_comp,
                   digest_len);
            session->session_id_len = digest_len;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "session_id calculated"));
        }

        /* Cleanup any existing cipher */
        if(session->local.crypt->dtor) {
            session->local.crypt->dtor(session,
                                       &session->local.crypt_abstract);
        }

        /* Calculate IV/Secret/Key for each direction */
        if(session->local.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &iv,
                                         session->local.crypt->iv_len,
                                         (const unsigned char *)"A");

            if(!iv) {
                ret = -1;
                goto clean_exit;
            }
            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &secret,
                                         session->local.crypt->secret_len,
                                         (const unsigned char *)"C");

            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->local.crypt->
                init(session, session->local.crypt, iv, &free_iv, secret,
                     &free_secret, 1, &session->local.crypt_abstract)) {
                LIBSSH2_FREE(session, iv);
                LIBSSH2_FREE(session, secret);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->local.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->local.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server IV and Key calculated"));

        if(session->remote.crypt->dtor) {
            /* Cleanup any existing cipher */
            session->remote.crypt->dtor(session,
                                        &session->remote.crypt_abstract);
        }

        if(session->remote.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &iv,
                                         session->remote.crypt->iv_len,
                                         (const unsigned char *)"B");
            if(!iv) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &secret,
                                         session->remote.crypt->secret_len,
                                         (const unsigned char *)"D");
            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->remote.crypt->
                init(session, session->remote.crypt, iv, &free_iv, secret,
                     &free_secret, 0, &session->remote.crypt_abstract)) {
                LIBSSH2_FREE(session, iv);
                LIBSSH2_FREE(session, secret);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->remote.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->remote.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client IV and Key calculated"));

        if(session->local.mac->dtor) {
            session->local.mac->dtor(session, &session->local.mac_abstract);
        }

        if(session->local.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &key,
                                         session->local.mac->key_len,
                                         (const unsigned char *)"E");
            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->local.mac->init(session, key, &free_key,
                                     &session->local.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->local.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server HMAC Key calculated"));

        if(session->remote.mac->dtor) {
            session->remote.mac->dtor(session, &session->remote.mac_abstract);
        }

        if(session->remote.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            _libssh2_sha_algo_value_hash(sha_algo_value, session,
                                         exchange_state, &key,
                                         session->remote.mac->key_len,
                                         (const unsigned char *)"F");
            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->remote.mac->init(session, key, &free_key,
                                      &session->remote.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->remote.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client HMAC Key calculated"));

        /* Initialize compression for each direction */

        /* Cleanup any existing compression */
        if(session->local.comp && session->local.comp->dtor) {
            session->local.comp->dtor(session, 1,
                                      &session->local.comp_abstract);
        }

        if(session->local.comp && session->local.comp->init) {
            if(session->local.comp->init(session, 1,
                                         &session->local.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server compression initialized"));

        if(session->remote.comp && session->remote.comp->dtor) {
            session->remote.comp->dtor(session, 0,
                                       &session->remote.comp_abstract);
        }

        if(session->remote.comp && session->remote.comp->init) {
            if(session->remote.comp->init(session, 0,
                                          &session->remote.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client compression initialized"));

    }

clean_exit:
    diffie_hellman_state_cleanup(session, exchange_state);

    return ret;
}



/* kex_method_diffie_hellman_group1_sha1_key_exchange
 * Diffie-Hellman Group1 (Actually Group2) Key Exchange using SHA1
 */
static int
kex_method_diffie_hellman_group1_sha1_key_exchange(LIBSSH2_SESSION *session,
                                                   key_exchange_state_low_t
                                                   * key_state)
{
    static const unsigned char p_value[128] = {
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68, 0xC2, 0x34,
        0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1,
        0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74,
        0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22,
        0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
        0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B,
        0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37,
        0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45,
        0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6,
        0xF4, 0x4C, 0x42, 0xE9, 0xA6, 0x37, 0xED, 0x6B,
        0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
        0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5,
        0xAE, 0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6,
        0x49, 0x28, 0x66, 0x51, 0xEC, 0xE6, 0x53, 0x81,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF
    };

    int ret;
    libssh2_sha1_ctx exchange_hash_ctx;

    if(key_state->state == libssh2_NB_state_idle) {
        /* g == 2 */
        key_state->p = _libssh2_bn_init_from_bin(); /* SSH2 defined value
                                                       (p_value) */
        key_state->g = _libssh2_bn_init();      /* SSH2 defined value (2) */

        /* Initialize P and G */
        if(!key_state->g || _libssh2_bn_set_word(key_state->g, 2)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state g.");
            goto clean_exit;
        }
        if(!key_state->p || _libssh2_bn_from_bin(key_state->p, 128, p_value)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state p.");
            goto clean_exit;
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group1 Key Exchange"));

        key_state->state = libssh2_NB_state_created;
    }

    ret = diffie_hellman_sha_algo(session, key_state->g, key_state->p, 128, 1,
                                  (void *)&exchange_hash_ctx,
                                  SSH_MSG_KEXDH_INIT, SSH_MSG_KEXDH_REPLY,
                                  NULL, 0, &key_state->exchange_state);
    if(ret == LIBSSH2_ERROR_EAGAIN) {
        return ret;
    }

clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}


/* kex_method_diffie_hellman_group14_key_exchange
 * Diffie-Hellman Group14 Key Exchange with hash function callback
 */
typedef int (*diffie_hellman_hash_func_t)(LIBSSH2_SESSION *,
                                          _libssh2_bn *,
                                          _libssh2_bn *,
                                          int,
                                          int,
                                          void *,
                                          unsigned char,
                                          unsigned char,
                                          unsigned char *,
                                          size_t,
                                          kmdhgGPshakex_state_t *);
static int
kex_method_diffie_hellman_group14_key_exchange(LIBSSH2_SESSION *session,
                                               key_exchange_state_low_t
                                               * key_state,
                                               int sha_algo_value,
                                               void *exchange_hash_ctx,
                                               diffie_hellman_hash_func_t
                                               hashfunc)
{
    static const unsigned char p_value[256] = {
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68, 0xC2, 0x34,
        0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1,
        0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74,
        0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22,
        0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
        0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B,
        0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37,
        0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45,
        0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6,
        0xF4, 0x4C, 0x42, 0xE9, 0xA6, 0x37, 0xED, 0x6B,
        0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
        0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5,
        0xAE, 0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6,
        0x49, 0x28, 0x66, 0x51, 0xEC, 0xE4, 0x5B, 0x3D,
        0xC2, 0x00, 0x7C, 0xB8, 0xA1, 0x63, 0xBF, 0x05,
        0x98, 0xDA, 0x48, 0x36, 0x1C, 0x55, 0xD3, 0x9A,
        0x69, 0x16, 0x3F, 0xA8, 0xFD, 0x24, 0xCF, 0x5F,
        0x83, 0x65, 0x5D, 0x23, 0xDC, 0xA3, 0xAD, 0x96,
        0x1C, 0x62, 0xF3, 0x56, 0x20, 0x85, 0x52, 0xBB,
        0x9E, 0xD5, 0x29, 0x07, 0x70, 0x96, 0x96, 0x6D,
        0x67, 0x0C, 0x35, 0x4E, 0x4A, 0xBC, 0x98, 0x04,
        0xF1, 0x74, 0x6C, 0x08, 0xCA, 0x18, 0x21, 0x7C,
        0x32, 0x90, 0x5E, 0x46, 0x2E, 0x36, 0xCE, 0x3B,
        0xE3, 0x9E, 0x77, 0x2C, 0x18, 0x0E, 0x86, 0x03,
        0x9B, 0x27, 0x83, 0xA2, 0xEC, 0x07, 0xA2, 0x8F,
        0xB5, 0xC5, 0x5D, 0xF0, 0x6F, 0x4C, 0x52, 0xC9,
        0xDE, 0x2B, 0xCB, 0xF6, 0x95, 0x58, 0x17, 0x18,
        0x39, 0x95, 0x49, 0x7C, 0xEA, 0x95, 0x6A, 0xE5,
        0x15, 0xD2, 0x26, 0x18, 0x98, 0xFA, 0x05, 0x10,
        0x15, 0x72, 0x8E, 0x5A, 0x8A, 0xAC, 0xAA, 0x68,
        0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF
    };
    int ret;

    if(key_state->state == libssh2_NB_state_idle) {
        key_state->p = _libssh2_bn_init_from_bin(); /* SSH2 defined value
                                                       (p_value) */
        key_state->g = _libssh2_bn_init();      /* SSH2 defined value (2) */

        /* g == 2 */
        /* Initialize P and G */
        if(!key_state->g || _libssh2_bn_set_word(key_state->g, 2)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state g.");
            goto clean_exit;
        }
        else if(!key_state->p ||
                _libssh2_bn_from_bin(key_state->p, 256, p_value)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state p.");
            goto clean_exit;
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group14 Key Exchange"));

        key_state->state = libssh2_NB_state_created;
    }
    ret = hashfunc(session, key_state->g, key_state->p,
                   256, sha_algo_value, exchange_hash_ctx, SSH_MSG_KEXDH_INIT,
                   SSH_MSG_KEXDH_REPLY, NULL, 0, &key_state->exchange_state);
    if(ret == LIBSSH2_ERROR_EAGAIN) {
        return ret;
    }

clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}



/* kex_method_diffie_hellman_group14_sha1_key_exchange
 * Diffie-Hellman Group14 Key Exchange using SHA1
 */
static int
kex_method_diffie_hellman_group14_sha1_key_exchange(LIBSSH2_SESSION *session,
                                                    key_exchange_state_low_t
                                                    * key_state)
{
    libssh2_sha1_ctx ctx;
    return kex_method_diffie_hellman_group14_key_exchange(session,
                                                      key_state, 1,
                                                      &ctx,
                                                      diffie_hellman_sha_algo);
}



/* kex_method_diffie_hellman_group14_sha256_key_exchange
 * Diffie-Hellman Group14 Key Exchange using SHA256
 */
static int
kex_method_diffie_hellman_group14_sha256_key_exchange(LIBSSH2_SESSION *session,
                                                      key_exchange_state_low_t
                                                      * key_state)
{
    libssh2_sha256_ctx ctx;
    return kex_method_diffie_hellman_group14_key_exchange(session,
                                                      key_state, 256,
                                                      &ctx,
                                                      diffie_hellman_sha_algo);
}

/* kex_method_diffie_hellman_group16_sha512_key_exchange
* Diffie-Hellman Group16 Key Exchange using SHA512
*/
static int
kex_method_diffie_hellman_group16_sha512_key_exchange(LIBSSH2_SESSION *session,
                                                      key_exchange_state_low_t
                                                      * key_state)
{
    static const unsigned char p_value[512] = {
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2,
    0x21, 0x68, 0xC2, 0x34, 0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1,
    0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74, 0x02, 0x0B, 0xBE, 0xA6,
    0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
    0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B, 0x30, 0x2B, 0x0A, 0x6D,
    0xF2, 0x5F, 0x14, 0x37, 0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45,
    0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6, 0xF4, 0x4C, 0x42, 0xE9,
    0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
    0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE, 0x9F, 0x24, 0x11,
    0x7C, 0x4B, 0x1F, 0xE6, 0x49, 0x28, 0x66, 0x51, 0xEC, 0xE4, 0x5B, 0x3D,
    0xC2, 0x00, 0x7C, 0xB8, 0xA1, 0x63, 0xBF, 0x05, 0x98, 0xDA, 0x48, 0x36,
    0x1C, 0x55, 0xD3, 0x9A, 0x69, 0x16, 0x3F, 0xA8, 0xFD, 0x24, 0xCF, 0x5F,
    0x83, 0x65, 0x5D, 0x23, 0xDC, 0xA3, 0xAD, 0x96, 0x1C, 0x62, 0xF3, 0x56,
    0x20, 0x85, 0x52, 0xBB, 0x9E, 0xD5, 0x29, 0x07, 0x70, 0x96, 0x96, 0x6D,
    0x67, 0x0C, 0x35, 0x4E, 0x4A, 0xBC, 0x98, 0x04, 0xF1, 0x74, 0x6C, 0x08,
    0xCA, 0x18, 0x21, 0x7C, 0x32, 0x90, 0x5E, 0x46, 0x2E, 0x36, 0xCE, 0x3B,
    0xE3, 0x9E, 0x77, 0x2C, 0x18, 0x0E, 0x86, 0x03, 0x9B, 0x27, 0x83, 0xA2,
    0xEC, 0x07, 0xA2, 0x8F, 0xB5, 0xC5, 0x5D, 0xF0, 0x6F, 0x4C, 0x52, 0xC9,
    0xDE, 0x2B, 0xCB, 0xF6, 0x95, 0x58, 0x17, 0x18, 0x39, 0x95, 0x49, 0x7C,
    0xEA, 0x95, 0x6A, 0xE5, 0x15, 0xD2, 0x26, 0x18, 0x98, 0xFA, 0x05, 0x10,
    0x15, 0x72, 0x8E, 0x5A, 0x8A, 0xAA, 0xC4, 0x2D, 0xAD, 0x33, 0x17, 0x0D,
    0x04, 0x50, 0x7A, 0x33, 0xA8, 0x55, 0x21, 0xAB, 0xDF, 0x1C, 0xBA, 0x64,
    0xEC, 0xFB, 0x85, 0x04, 0x58, 0xDB, 0xEF, 0x0A, 0x8A, 0xEA, 0x71, 0x57,
    0x5D, 0x06, 0x0C, 0x7D, 0xB3, 0x97, 0x0F, 0x85, 0xA6, 0xE1, 0xE4, 0xC7,
    0xAB, 0xF5, 0xAE, 0x8C, 0xDB, 0x09, 0x33, 0xD7, 0x1E, 0x8C, 0x94, 0xE0,
    0x4A, 0x25, 0x61, 0x9D, 0xCE, 0xE3, 0xD2, 0x26, 0x1A, 0xD2, 0xEE, 0x6B,
    0xF1, 0x2F, 0xFA, 0x06, 0xD9, 0x8A, 0x08, 0x64, 0xD8, 0x76, 0x02, 0x73,
    0x3E, 0xC8, 0x6A, 0x64, 0x52, 0x1F, 0x2B, 0x18, 0x17, 0x7B, 0x20, 0x0C,
    0xBB, 0xE1, 0x17, 0x57, 0x7A, 0x61, 0x5D, 0x6C, 0x77, 0x09, 0x88, 0xC0,
    0xBA, 0xD9, 0x46, 0xE2, 0x08, 0xE2, 0x4F, 0xA0, 0x74, 0xE5, 0xAB, 0x31,
    0x43, 0xDB, 0x5B, 0xFC, 0xE0, 0xFD, 0x10, 0x8E, 0x4B, 0x82, 0xD1, 0x20,
    0xA9, 0x21, 0x08, 0x01, 0x1A, 0x72, 0x3C, 0x12, 0xA7, 0x87, 0xE6, 0xD7,
    0x88, 0x71, 0x9A, 0x10, 0xBD, 0xBA, 0x5B, 0x26, 0x99, 0xC3, 0x27, 0x18,
    0x6A, 0xF4, 0xE2, 0x3C, 0x1A, 0x94, 0x68, 0x34, 0xB6, 0x15, 0x0B, 0xDA,
    0x25, 0x83, 0xE9, 0xCA, 0x2A, 0xD4, 0x4C, 0xE8, 0xDB, 0xBB, 0xC2, 0xDB,
    0x04, 0xDE, 0x8E, 0xF9, 0x2E, 0x8E, 0xFC, 0x14, 0x1F, 0xBE, 0xCA, 0xA6,
    0x28, 0x7C, 0x59, 0x47, 0x4E, 0x6B, 0xC0, 0x5D, 0x99, 0xB2, 0x96, 0x4F,
    0xA0, 0x90, 0xC3, 0xA2, 0x23, 0x3B, 0xA1, 0x86, 0x51, 0x5B, 0xE7, 0xED,
    0x1F, 0x61, 0x29, 0x70, 0xCE, 0xE2, 0xD7, 0xAF, 0xB8, 0x1B, 0xDD, 0x76,
    0x21, 0x70, 0x48, 0x1C, 0xD0, 0x06, 0x91, 0x27, 0xD5, 0xB0, 0x5A, 0xA9,
    0x93, 0xB4, 0xEA, 0x98, 0x8D, 0x8F, 0xDD, 0xC1, 0x86, 0xFF, 0xB7, 0xDC,
    0x90, 0xA6, 0xC0, 0x8F, 0x4D, 0xF4, 0x35, 0xC9, 0x34, 0x06, 0x31, 0x99,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF
    };
    int ret;
    libssh2_sha512_ctx exchange_hash_ctx;

    if(key_state->state == libssh2_NB_state_idle) {
        key_state->p = _libssh2_bn_init_from_bin(); /* SSH2 defined value
                                                       (p_value) */
        key_state->g = _libssh2_bn_init();      /* SSH2 defined value (2) */

        /* g == 2 */
        /* Initialize P and G */
        if(!key_state->g || _libssh2_bn_set_word(key_state->g, 2)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state g.");
            goto clean_exit;
        }
        if(!key_state->p || _libssh2_bn_from_bin(key_state->p, 512, p_value)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state p.");
            goto clean_exit;
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group16 Key Exchange"));

        key_state->state = libssh2_NB_state_created;
    }

    ret = diffie_hellman_sha_algo(session, key_state->g, key_state->p, 512,
                                  512, (void *)&exchange_hash_ctx,
                                  SSH_MSG_KEXDH_INIT, SSH_MSG_KEXDH_REPLY,
                                  NULL, 0, &key_state->exchange_state);
    if(ret == LIBSSH2_ERROR_EAGAIN) {
        return ret;
    }

clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}

/* kex_method_diffie_hellman_group16_sha512_key_exchange
* Diffie-Hellman Group18 Key Exchange using SHA512
*/
static int
kex_method_diffie_hellman_group18_sha512_key_exchange(LIBSSH2_SESSION *session,
                                                      key_exchange_state_low_t
                                                      * key_state)
{
    static const unsigned char p_value[1024] = {
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2,
    0x21, 0x68, 0xC2, 0x34, 0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1,
    0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74, 0x02, 0x0B, 0xBE, 0xA6,
    0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
    0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B, 0x30, 0x2B, 0x0A, 0x6D,
    0xF2, 0x5F, 0x14, 0x37, 0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45,
    0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6, 0xF4, 0x4C, 0x42, 0xE9,
    0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
    0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE, 0x9F, 0x24, 0x11,
    0x7C, 0x4B, 0x1F, 0xE6, 0x49, 0x28, 0x66, 0x51, 0xEC, 0xE4, 0x5B, 0x3D,
    0xC2, 0x00, 0x7C, 0xB8, 0xA1, 0x63, 0xBF, 0x05, 0x98, 0xDA, 0x48, 0x36,
    0x1C, 0x55, 0xD3, 0x9A, 0x69, 0x16, 0x3F, 0xA8, 0xFD, 0x24, 0xCF, 0x5F,
    0x83, 0x65, 0x5D, 0x23, 0xDC, 0xA3, 0xAD, 0x96, 0x1C, 0x62, 0xF3, 0x56,
    0x20, 0x85, 0x52, 0xBB, 0x9E, 0xD5, 0x29, 0x07, 0x70, 0x96, 0x96, 0x6D,
    0x67, 0x0C, 0x35, 0x4E, 0x4A, 0xBC, 0x98, 0x04, 0xF1, 0x74, 0x6C, 0x08,
    0xCA, 0x18, 0x21, 0x7C, 0x32, 0x90, 0x5E, 0x46, 0x2E, 0x36, 0xCE, 0x3B,
    0xE3, 0x9E, 0x77, 0x2C, 0x18, 0x0E, 0x86, 0x03, 0x9B, 0x27, 0x83, 0xA2,
    0xEC, 0x07, 0xA2, 0x8F, 0xB5, 0xC5, 0x5D, 0xF0, 0x6F, 0x4C, 0x52, 0xC9,
    0xDE, 0x2B, 0xCB, 0xF6, 0x95, 0x58, 0x17, 0x18, 0x39, 0x95, 0x49, 0x7C,
    0xEA, 0x95, 0x6A, 0xE5, 0x15, 0xD2, 0x26, 0x18, 0x98, 0xFA, 0x05, 0x10,
    0x15, 0x72, 0x8E, 0x5A, 0x8A, 0xAA, 0xC4, 0x2D, 0xAD, 0x33, 0x17, 0x0D,
    0x04, 0x50, 0x7A, 0x33, 0xA8, 0x55, 0x21, 0xAB, 0xDF, 0x1C, 0xBA, 0x64,
    0xEC, 0xFB, 0x85, 0x04, 0x58, 0xDB, 0xEF, 0x0A, 0x8A, 0xEA, 0x71, 0x57,
    0x5D, 0x06, 0x0C, 0x7D, 0xB3, 0x97, 0x0F, 0x85, 0xA6, 0xE1, 0xE4, 0xC7,
    0xAB, 0xF5, 0xAE, 0x8C, 0xDB, 0x09, 0x33, 0xD7, 0x1E, 0x8C, 0x94, 0xE0,
    0x4A, 0x25, 0x61, 0x9D, 0xCE, 0xE3, 0xD2, 0x26, 0x1A, 0xD2, 0xEE, 0x6B,
    0xF1, 0x2F, 0xFA, 0x06, 0xD9, 0x8A, 0x08, 0x64, 0xD8, 0x76, 0x02, 0x73,
    0x3E, 0xC8, 0x6A, 0x64, 0x52, 0x1F, 0x2B, 0x18, 0x17, 0x7B, 0x20, 0x0C,
    0xBB, 0xE1, 0x17, 0x57, 0x7A, 0x61, 0x5D, 0x6C, 0x77, 0x09, 0x88, 0xC0,
    0xBA, 0xD9, 0x46, 0xE2, 0x08, 0xE2, 0x4F, 0xA0, 0x74, 0xE5, 0xAB, 0x31,
    0x43, 0xDB, 0x5B, 0xFC, 0xE0, 0xFD, 0x10, 0x8E, 0x4B, 0x82, 0xD1, 0x20,
    0xA9, 0x21, 0x08, 0x01, 0x1A, 0x72, 0x3C, 0x12, 0xA7, 0x87, 0xE6, 0xD7,
    0x88, 0x71, 0x9A, 0x10, 0xBD, 0xBA, 0x5B, 0x26, 0x99, 0xC3, 0x27, 0x18,
    0x6A, 0xF4, 0xE2, 0x3C, 0x1A, 0x94, 0x68, 0x34, 0xB6, 0x15, 0x0B, 0xDA,
    0x25, 0x83, 0xE9, 0xCA, 0x2A, 0xD4, 0x4C, 0xE8, 0xDB, 0xBB, 0xC2, 0xDB,
    0x04, 0xDE, 0x8E, 0xF9, 0x2E, 0x8E, 0xFC, 0x14, 0x1F, 0xBE, 0xCA, 0xA6,
    0x28, 0x7C, 0x59, 0x47, 0x4E, 0x6B, 0xC0, 0x5D, 0x99, 0xB2, 0x96, 0x4F,
    0xA0, 0x90, 0xC3, 0xA2, 0x23, 0x3B, 0xA1, 0x86, 0x51, 0x5B, 0xE7, 0xED,
    0x1F, 0x61, 0x29, 0x70, 0xCE, 0xE2, 0xD7, 0xAF, 0xB8, 0x1B, 0xDD, 0x76,
    0x21, 0x70, 0x48, 0x1C, 0xD0, 0x06, 0x91, 0x27, 0xD5, 0xB0, 0x5A, 0xA9,
    0x93, 0xB4, 0xEA, 0x98, 0x8D, 0x8F, 0xDD, 0xC1, 0x86, 0xFF, 0xB7, 0xDC,
    0x90, 0xA6, 0xC0, 0x8F, 0x4D, 0xF4, 0x35, 0xC9, 0x34, 0x02, 0x84, 0x92,
    0x36, 0xC3, 0xFA, 0xB4, 0xD2, 0x7C, 0x70, 0x26, 0xC1, 0xD4, 0xDC, 0xB2,
    0x60, 0x26, 0x46, 0xDE, 0xC9, 0x75, 0x1E, 0x76, 0x3D, 0xBA, 0x37, 0xBD,
    0xF8, 0xFF, 0x94, 0x06, 0xAD, 0x9E, 0x53, 0x0E, 0xE5, 0xDB, 0x38, 0x2F,
    0x41, 0x30, 0x01, 0xAE, 0xB0, 0x6A, 0x53, 0xED, 0x90, 0x27, 0xD8, 0x31,
    0x17, 0x97, 0x27, 0xB0, 0x86, 0x5A, 0x89, 0x18, 0xDA, 0x3E, 0xDB, 0xEB,
    0xCF, 0x9B, 0x14, 0xED, 0x44, 0xCE, 0x6C, 0xBA, 0xCE, 0xD4, 0xBB, 0x1B,
    0xDB, 0x7F, 0x14, 0x47, 0xE6, 0xCC, 0x25, 0x4B, 0x33, 0x20, 0x51, 0x51,
    0x2B, 0xD7, 0xAF, 0x42, 0x6F, 0xB8, 0xF4, 0x01, 0x37, 0x8C, 0xD2, 0xBF,
    0x59, 0x83, 0xCA, 0x01, 0xC6, 0x4B, 0x92, 0xEC, 0xF0, 0x32, 0xEA, 0x15,
    0xD1, 0x72, 0x1D, 0x03, 0xF4, 0x82, 0xD7, 0xCE, 0x6E, 0x74, 0xFE, 0xF6,
    0xD5, 0x5E, 0x70, 0x2F, 0x46, 0x98, 0x0C, 0x82, 0xB5, 0xA8, 0x40, 0x31,
    0x90, 0x0B, 0x1C, 0x9E, 0x59, 0xE7, 0xC9, 0x7F, 0xBE, 0xC7, 0xE8, 0xF3,
    0x23, 0xA9, 0x7A, 0x7E, 0x36, 0xCC, 0x88, 0xBE, 0x0F, 0x1D, 0x45, 0xB7,
    0xFF, 0x58, 0x5A, 0xC5, 0x4B, 0xD4, 0x07, 0xB2, 0x2B, 0x41, 0x54, 0xAA,
    0xCC, 0x8F, 0x6D, 0x7E, 0xBF, 0x48, 0xE1, 0xD8, 0x14, 0xCC, 0x5E, 0xD2,
    0x0F, 0x80, 0x37, 0xE0, 0xA7, 0x97, 0x15, 0xEE, 0xF2, 0x9B, 0xE3, 0x28,
    0x06, 0xA1, 0xD5, 0x8B, 0xB7, 0xC5, 0xDA, 0x76, 0xF5, 0x50, 0xAA, 0x3D,
    0x8A, 0x1F, 0xBF, 0xF0, 0xEB, 0x19, 0xCC, 0xB1, 0xA3, 0x13, 0xD5, 0x5C,
    0xDA, 0x56, 0xC9, 0xEC, 0x2E, 0xF2, 0x96, 0x32, 0x38, 0x7F, 0xE8, 0xD7,
    0x6E, 0x3C, 0x04, 0x68, 0x04, 0x3E, 0x8F, 0x66, 0x3F, 0x48, 0x60, 0xEE,
    0x12, 0xBF, 0x2D, 0x5B, 0x0B, 0x74, 0x74, 0xD6, 0xE6, 0x94, 0xF9, 0x1E,
    0x6D, 0xBE, 0x11, 0x59, 0x74, 0xA3, 0x92, 0x6F, 0x12, 0xFE, 0xE5, 0xE4,
    0x38, 0x77, 0x7C, 0xB6, 0xA9, 0x32, 0xDF, 0x8C, 0xD8, 0xBE, 0xC4, 0xD0,
    0x73, 0xB9, 0x31, 0xBA, 0x3B, 0xC8, 0x32, 0xB6, 0x8D, 0x9D, 0xD3, 0x00,
    0x74, 0x1F, 0xA7, 0xBF, 0x8A, 0xFC, 0x47, 0xED, 0x25, 0x76, 0xF6, 0x93,
    0x6B, 0xA4, 0x24, 0x66, 0x3A, 0xAB, 0x63, 0x9C, 0x5A, 0xE4, 0xF5, 0x68,
    0x34, 0x23, 0xB4, 0x74, 0x2B, 0xF1, 0xC9, 0x78, 0x23, 0x8F, 0x16, 0xCB,
    0xE3, 0x9D, 0x65, 0x2D, 0xE3, 0xFD, 0xB8, 0xBE, 0xFC, 0x84, 0x8A, 0xD9,
    0x22, 0x22, 0x2E, 0x04, 0xA4, 0x03, 0x7C, 0x07, 0x13, 0xEB, 0x57, 0xA8,
    0x1A, 0x23, 0xF0, 0xC7, 0x34, 0x73, 0xFC, 0x64, 0x6C, 0xEA, 0x30, 0x6B,
    0x4B, 0xCB, 0xC8, 0x86, 0x2F, 0x83, 0x85, 0xDD, 0xFA, 0x9D, 0x4B, 0x7F,
    0xA2, 0xC0, 0x87, 0xE8, 0x79, 0x68, 0x33, 0x03, 0xED, 0x5B, 0xDD, 0x3A,
    0x06, 0x2B, 0x3C, 0xF5, 0xB3, 0xA2, 0x78, 0xA6, 0x6D, 0x2A, 0x13, 0xF8,
    0x3F, 0x44, 0xF8, 0x2D, 0xDF, 0x31, 0x0E, 0xE0, 0x74, 0xAB, 0x6A, 0x36,
    0x45, 0x97, 0xE8, 0x99, 0xA0, 0x25, 0x5D, 0xC1, 0x64, 0xF3, 0x1C, 0xC5,
    0x08, 0x46, 0x85, 0x1D, 0xF9, 0xAB, 0x48, 0x19, 0x5D, 0xED, 0x7E, 0xA1,
    0xB1, 0xD5, 0x10, 0xBD, 0x7E, 0xE7, 0x4D, 0x73, 0xFA, 0xF3, 0x6B, 0xC3,
    0x1E, 0xCF, 0xA2, 0x68, 0x35, 0x90, 0x46, 0xF4, 0xEB, 0x87, 0x9F, 0x92,
    0x40, 0x09, 0x43, 0x8B, 0x48, 0x1C, 0x6C, 0xD7, 0x88, 0x9A, 0x00, 0x2E,
    0xD5, 0xEE, 0x38, 0x2B, 0xC9, 0x19, 0x0D, 0xA6, 0xFC, 0x02, 0x6E, 0x47,
    0x95, 0x58, 0xE4, 0x47, 0x56, 0x77, 0xE9, 0xAA, 0x9E, 0x30, 0x50, 0xE2,
    0x76, 0x56, 0x94, 0xDF, 0xC8, 0x1F, 0x56, 0xE8, 0x80, 0xB9, 0x6E, 0x71,
    0x60, 0xC9, 0x80, 0xDD, 0x98, 0xED, 0xD3, 0xDF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF
    };
    int ret;
    libssh2_sha512_ctx exchange_hash_ctx;

    if(key_state->state == libssh2_NB_state_idle) {
        key_state->p = _libssh2_bn_init_from_bin(); /* SSH2 defined value
                                                       (p_value) */
        key_state->g = _libssh2_bn_init();      /* SSH2 defined value (2) */

        /* g == 2 */
        /* Initialize P and G */
        if(!key_state->g || _libssh2_bn_set_word(key_state->g, 2)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state g.");
            goto clean_exit;
        }
        else if(!key_state->p ||
                _libssh2_bn_from_bin(key_state->p, 1024, p_value)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Failed to allocate key state p.");
            goto clean_exit;
        }

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group18 Key Exchange"));

        key_state->state = libssh2_NB_state_created;
    }

    ret = diffie_hellman_sha_algo(session, key_state->g, key_state->p, 1024,
                                  512, (void *)&exchange_hash_ctx,
                                  SSH_MSG_KEXDH_INIT, SSH_MSG_KEXDH_REPLY,
                                  NULL, 0, &key_state->exchange_state);
    if(ret == LIBSSH2_ERROR_EAGAIN) {
        return ret;
    }

clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}

/* kex_method_diffie_hellman_group_exchange_sha1_key_exchange
 * Diffie-Hellman Group Exchange Key Exchange using SHA1
 * Negotiates random(ish) group for secret derivation
 */
static int
kex_method_diffie_hellman_group_exchange_sha1_key_exchange(
                                          LIBSSH2_SESSION * session,
                                          key_exchange_state_low_t * key_state)
{
    int ret = 0;
    int rc;

    if(key_state->state == libssh2_NB_state_idle) {
        key_state->p = _libssh2_bn_init_from_bin();
        key_state->g = _libssh2_bn_init_from_bin();
        /* Ask for a P and G pair */
        key_state->request[0] = SSH_MSG_KEX_DH_GEX_REQUEST;
        _libssh2_htonu32(key_state->request + 1, LIBSSH2_DH_GEX_MINGROUP);
        _libssh2_htonu32(key_state->request + 5, LIBSSH2_DH_GEX_OPTGROUP);
        _libssh2_htonu32(key_state->request + 9, LIBSSH2_DH_GEX_MAXGROUP);
        key_state->request_len = 13;
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group-Exchange SHA1"));

        key_state->state = libssh2_NB_state_created;
    }

    if(key_state->state == libssh2_NB_state_created) {
        rc = _libssh2_transport_send(session, key_state->request,
                                     key_state->request_len, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send Group Exchange Request");
            goto dh_gex_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent;
    }

    if(key_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_packet_require(session, SSH_MSG_KEX_DH_GEX_GROUP,
                                     &key_state->data, &key_state->data_len,
                                     0, NULL, 0, &key_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timeout waiting for GEX_GROUP reply");
            goto dh_gex_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent1;
    }

    if(key_state->state == libssh2_NB_state_sent1) {
        size_t p_len, g_len;
        unsigned char *p, *g;
        struct string_buf buf;
        libssh2_sha1_ctx exchange_hash_ctx;

        if(key_state->data_len < 9) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected key length DH-SHA1");
            goto dh_gex_clean_exit;
        }

        buf.data = key_state->data;
        buf.dataptr = buf.data;
        buf.len = key_state->data_len;

        buf.dataptr++; /* increment to big num */

        if(_libssh2_get_bignum_bytes(&buf, &p, &p_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected value DH-SHA1 p");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_get_bignum_bytes(&buf, &g, &g_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected value DH-SHA1 g");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_bn_from_bin(key_state->p, p_len, p)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Invalid DH-SHA1 p");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_bn_from_bin(key_state->g, g_len, g)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Invalid DH-SHA1 g");
            goto dh_gex_clean_exit;
        }

        ret = diffie_hellman_sha_algo(session, key_state->g, key_state->p,
                                      (int)p_len, 1,
                                      (void *)&exchange_hash_ctx,
                                      SSH_MSG_KEX_DH_GEX_INIT,
                                      SSH_MSG_KEX_DH_GEX_REPLY,
                                      key_state->data + 1,
                                      key_state->data_len - 1,
                                      &key_state->exchange_state);
        if(ret == LIBSSH2_ERROR_EAGAIN) {
            return ret;
        }
    }

dh_gex_clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}



/* kex_method_diffie_hellman_group_exchange_sha256_key_exchange
 * Diffie-Hellman Group Exchange Key Exchange using SHA256
 * Negotiates random(ish) group for secret derivation
 */
static int
kex_method_diffie_hellman_group_exchange_sha256_key_exchange(
                                          LIBSSH2_SESSION * session,
                                          key_exchange_state_low_t * key_state)
{
    int ret = 0;
    int rc;

    if(key_state->state == libssh2_NB_state_idle) {
        key_state->p = _libssh2_bn_init();
        key_state->g = _libssh2_bn_init();
        /* Ask for a P and G pair */
        key_state->request[0] = SSH_MSG_KEX_DH_GEX_REQUEST;
        _libssh2_htonu32(key_state->request + 1, LIBSSH2_DH_GEX_MINGROUP);
        _libssh2_htonu32(key_state->request + 5, LIBSSH2_DH_GEX_OPTGROUP);
        _libssh2_htonu32(key_state->request + 9, LIBSSH2_DH_GEX_MAXGROUP);
        key_state->request_len = 13;
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating Diffie-Hellman Group-Exchange SHA256"));

        key_state->state = libssh2_NB_state_created;
    }

    if(key_state->state == libssh2_NB_state_created) {
        rc = _libssh2_transport_send(session, key_state->request,
                                     key_state->request_len, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send "
                                 "Group Exchange Request SHA256");
            goto dh_gex_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent;
    }

    if(key_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_packet_require(session, SSH_MSG_KEX_DH_GEX_GROUP,
                                     &key_state->data, &key_state->data_len,
                                     0, NULL, 0, &key_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timeout waiting for GEX_GROUP reply SHA256");
            goto dh_gex_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent1;
    }

    if(key_state->state == libssh2_NB_state_sent1) {
        unsigned char *p, *g;
        size_t p_len, g_len;
        struct string_buf buf;
        libssh2_sha256_ctx exchange_hash_ctx;

        if(key_state->data_len < 9) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected key length DH-SHA256");
            goto dh_gex_clean_exit;
        }

        buf.data = key_state->data;
        buf.dataptr = buf.data;
        buf.len = key_state->data_len;

        buf.dataptr++; /* increment to big num */

        if(_libssh2_get_bignum_bytes(&buf, &p, &p_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected value DH-SHA256 p");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_get_bignum_bytes(&buf, &g, &g_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected value DH-SHA256 g");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_bn_from_bin(key_state->p, p_len, p)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Invalid DH-SHA256 p");
            goto dh_gex_clean_exit;
        }

        if(_libssh2_bn_from_bin(key_state->g, g_len, g)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Invalid DH-SHA256 g");
            goto dh_gex_clean_exit;
        }

        ret = diffie_hellman_sha_algo(session, key_state->g, key_state->p,
                                      (int)p_len, 256,
                                      (void *)&exchange_hash_ctx,
                                      SSH_MSG_KEX_DH_GEX_INIT,
                                      SSH_MSG_KEX_DH_GEX_REPLY,
                                      key_state->data + 1,
                                      key_state->data_len - 1,
                                      &key_state->exchange_state);
        if(ret == LIBSSH2_ERROR_EAGAIN) {
            return ret;
        }
    }

dh_gex_clean_exit:
    kex_diffie_hellman_cleanup(session, key_state);

    return ret;
}

#if LIBSSH2_ECDSA

/* LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY
 *
 * Macro that create and verifies EC SHA hash with a given digest bytes
 *
 * Payload format:
 *
 * string   V_C, client's identification string (CR and LF excluded)
 * string   V_S, server's identification string (CR and LF excluded)
 * string   I_C, payload of the client's SSH_MSG_KEXINIT
 * string   I_S, payload of the server's SSH_MSG_KEXINIT
 * string   K_S, server's public host key
 * string   Q_C, client's ephemeral public key octet string
 * string   Q_S, server's ephemeral public key octet string
 * mpint    K,   shared secret
 *
 */
#define LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY(digest_type)            \
do {                                                                         \
    libssh2_sha##digest_type##_ctx ctx;                                      \
    int hok;                                                                 \
    if(!libssh2_sha##digest_type##_init(&ctx)) {                             \
        rc = -1;                                                             \
        break;                                                               \
    }                                                                        \
    exchange_state->exchange_hash = (void *)&ctx;                            \
    hok = 1;                                                                 \
    if(session->local.banner) {                                              \
        _libssh2_htonu32(exchange_state->h_sig_comp,                         \
            (uint32_t)(strlen((char *) session->local.banner) - 2));         \
        hok &= libssh2_sha##digest_type##_update(ctx,                        \
                                                 exchange_state->h_sig_comp, \
                                                 4);                         \
        hok &= libssh2_sha##digest_type##_update(ctx,                        \
                                              (char *)session->local.banner, \
                                       strlen((char *)session->local.banner) \
                                       - 2);                                 \
    }                                                                        \
    else {                                                                   \
        _libssh2_htonu32(exchange_state->h_sig_comp,                         \
                         sizeof(LIBSSH2_SSH_DEFAULT_BANNER) - 1);            \
        hok &= libssh2_sha##digest_type##_update(ctx,                        \
                                          exchange_state->h_sig_comp, 4);    \
        hok &= libssh2_sha##digest_type##_update(ctx,                        \
                                                 LIBSSH2_SSH_DEFAULT_BANNER, \
                                          sizeof(LIBSSH2_SSH_DEFAULT_BANNER) \
                                          - 1);                              \
    }                                                                        \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
        (uint32_t)strlen((char *) session->remote.banner));                  \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx, session->remote.banner,    \
                                   strlen((char *)session->remote.banner));  \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
                     (uint32_t)session->local.kexinit_len);                  \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             session->local.kexinit,         \
                                             session->local.kexinit_len);    \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
                     (uint32_t)session->remote.kexinit_len);                 \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             session->remote.kexinit,        \
                                             session->remote.kexinit_len);   \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
                     session->server_hostkey_len);                           \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             session->server_hostkey,        \
                                             session->server_hostkey_len);   \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
                     (uint32_t)public_key_len);                              \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             public_key,                     \
                                             public_key_len);                \
                                                                             \
    _libssh2_htonu32(exchange_state->h_sig_comp,                             \
                     (uint32_t)server_public_key_len);                       \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->h_sig_comp, 4); \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             server_public_key,              \
                                             server_public_key_len);         \
                                                                             \
    hok &= libssh2_sha##digest_type##_update(ctx,                            \
                                             exchange_state->k_value,        \
                                             exchange_state->k_value_len);   \
                                                                             \
    if(!hok ||                                                               \
       !libssh2_sha##digest_type##_final(ctx, exchange_state->h_sig_comp)) { \
        rc = -1;                                                             \
        break;                                                               \
    }                                                                        \
                                                                             \
    if(session->hostkey->                                                    \
        sig_verify(session, exchange_state->h_sig,                           \
                   exchange_state->h_sig_len, exchange_state->h_sig_comp,    \
                   SHA##digest_type##_DIGEST_LENGTH,                         \
                   &session->server_hostkey_abstract)) {                     \
        rc = -1;                                                             \
    }                                                                        \
} while(0)

/* kex_session_ecdh_curve_type
 * returns the EC curve type by name used in key exchange
 */

static int
kex_session_ecdh_curve_type(const char *name, libssh2_curve_type *out_type)
{
    libssh2_curve_type type;

    if(!name)
        return -1;

    if(strcmp(name, "ecdh-sha2-nistp256") == 0)
        type = LIBSSH2_EC_CURVE_NISTP256;
    else if(strcmp(name, "ecdh-sha2-nistp384") == 0)
        type = LIBSSH2_EC_CURVE_NISTP384;
    else if(strcmp(name, "ecdh-sha2-nistp521") == 0)
        type = LIBSSH2_EC_CURVE_NISTP521;
    else {
        return -1;
    }

    if(out_type) {
        *out_type = type;
    }

    return 0;
}


static void
ecdh_exchange_state_cleanup(LIBSSH2_SESSION * session,
                            kmdhgGPshakex_state_t *exchange_state)
{
    _libssh2_bn_free(exchange_state->k);
    exchange_state->k = NULL;

    if(exchange_state->k_value) {
        LIBSSH2_FREE(session, exchange_state->k_value);
        exchange_state->k_value = NULL;
    }

    exchange_state->state = libssh2_NB_state_idle;
}


static void
kex_method_ecdh_cleanup
(LIBSSH2_SESSION * session, key_exchange_state_low_t * key_state)
{
    if(key_state->public_key_oct) {
        LIBSSH2_FREE(session, key_state->public_key_oct);
        key_state->public_key_oct = NULL;
    }

    if(key_state->private_key) {
        _libssh2_ecdsa_free(key_state->private_key);
        key_state->private_key = NULL;
    }

    if(key_state->data) {
        LIBSSH2_FREE(session, key_state->data);
        key_state->data = NULL;
    }

    key_state->state = libssh2_NB_state_idle;

    if(key_state->exchange_state.state != libssh2_NB_state_idle) {
        ecdh_exchange_state_cleanup(session, &key_state->exchange_state);
    }
}


/* ecdh_sha2_nistp
 * Elliptic Curve Diffie Hellman Key Exchange
 */

static int ecdh_sha2_nistp(LIBSSH2_SESSION *session, libssh2_curve_type type,
                           unsigned char *data, size_t data_len,
                           unsigned char *public_key,
                           size_t public_key_len, _libssh2_ec_key *private_key,
                           kmdhgGPshakex_state_t *exchange_state)
{
    int ret = 0;
    int rc;

    if(data_len < 5) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                             "Host key data is too short");
        return ret;
    }

    if(exchange_state->state == libssh2_NB_state_idle) {

        /* Setup initial values */
        exchange_state->k = _libssh2_bn_init();

        exchange_state->state = libssh2_NB_state_created;
    }

    if(exchange_state->state == libssh2_NB_state_created) {
        /* parse INIT reply data */

        /* host key K_S */
        unsigned char *server_public_key;
        size_t server_public_key_len;
        struct string_buf buf;

        buf.data = data;
        buf.len = data_len;
        buf.dataptr = buf.data;
        buf.dataptr++; /* Advance past packet type */

        if(_libssh2_copy_string(session, &buf, &(session->server_hostkey),
                                &server_public_key_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate memory for a copy "
                                 "of the host ECDH key");
            goto clean_exit;
        }

        session->server_hostkey_len = (uint32_t)server_public_key_len;

#if LIBSSH2_MD5
        {
            libssh2_md5_ctx fingerprint_ctx;

            if(libssh2_md5_init(&fingerprint_ctx) &&
               libssh2_md5_update(fingerprint_ctx, session->server_hostkey,
                                  session->server_hostkey_len) &&
               libssh2_md5_final(fingerprint_ctx,
                                 session->server_hostkey_md5)) {
                session->server_hostkey_md5_valid = TRUE;
            }
            else {
                session->server_hostkey_md5_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[50], *fprint = fingerprint;
            int i;
            for(i = 0; i < 16; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_md5[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's MD5 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */
#endif /* ! LIBSSH2_MD5 */

        {
            libssh2_sha1_ctx fingerprint_ctx;

            if(libssh2_sha1_init(&fingerprint_ctx) &&
               libssh2_sha1_update(fingerprint_ctx, session->server_hostkey,
                                   session->server_hostkey_len) &&
               libssh2_sha1_final(fingerprint_ctx,
                                  session->server_hostkey_sha1)) {
                session->server_hostkey_sha1_valid = TRUE;
            }
            else {
                session->server_hostkey_sha1_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[64], *fprint = fingerprint;
            int i;
            for(i = 0; i < 20; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_sha1[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's SHA1 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */

        /* SHA256 */
        {
            libssh2_sha256_ctx fingerprint_ctx;

            if(libssh2_sha256_init(&fingerprint_ctx) &&
               libssh2_sha256_update(fingerprint_ctx, session->server_hostkey,
                                     session->server_hostkey_len) &&
               libssh2_sha256_final(fingerprint_ctx,
                                    session->server_hostkey_sha256)) {
                session->server_hostkey_sha256_valid = TRUE;
            }
            else {
                session->server_hostkey_sha256_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char *base64Fingerprint = NULL;
            _libssh2_base64_encode(session,
                                   (const char *)
                                   session->server_hostkey_sha256,
                                   SHA256_DIGEST_LENGTH, &base64Fingerprint);
            if(base64Fingerprint) {
                _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                               "Server's SHA256 Fingerprint: %s",
                               base64Fingerprint));
                LIBSSH2_FREE(session, base64Fingerprint);
            }
        }
#endif /* LIBSSH2DEBUG */

        if(session->hostkey->init(session, session->server_hostkey,
                                  session->server_hostkey_len,
                                  &session->server_hostkey_abstract)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unable to initialize hostkey importer "
                                 "ECDH");
            goto clean_exit;
        }

        /* server public key Q_S */
        if(_libssh2_get_string(&buf, &server_public_key,
                               &server_public_key_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected key length ECDH");
            goto clean_exit;
        }

        /* server signature */
        if(_libssh2_get_string(&buf, &exchange_state->h_sig,
           &(exchange_state->h_sig_len))) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unexpected ECDH server sig length");
            goto clean_exit;
        }

        /* Compute the shared secret K */
        rc = _libssh2_ecdh_gen_k(&exchange_state->k, private_key,
                                 server_public_key, server_public_key_len);
        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_KEX_FAILURE,
                                 "Unable to create ECDH shared secret");
            goto clean_exit;
        }

        exchange_state->k_value_len = _libssh2_bn_bytes(exchange_state->k) + 5;
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            /* don't need leading 00 */
            exchange_state->k_value_len--;
        }
        exchange_state->k_value =
        LIBSSH2_ALLOC(session, exchange_state->k_value_len);
        if(!exchange_state->k_value) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate buffer for ECDH K");
            goto clean_exit;
        }
        _libssh2_htonu32(exchange_state->k_value,
                         (uint32_t)(exchange_state->k_value_len - 4));
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 4)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->k");
                goto clean_exit;
            }
        }
        else {
            exchange_state->k_value[4] = 0;
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 5)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->e");
                goto clean_exit;
            }
        }

        /* verify hash */

        switch(type) {
            case LIBSSH2_EC_CURVE_NISTP256:
                LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY(256);
                break;
            case LIBSSH2_EC_CURVE_NISTP384:
                LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY(384);
                break;
            case LIBSSH2_EC_CURVE_NISTP521:
                LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY(512);
                break;
        }

        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_SIGN,
                                 "Unable to verify hostkey signature "
                                 "ECDH");
            goto clean_exit;
        }

        exchange_state->c = SSH_MSG_NEWKEYS;
        exchange_state->state = libssh2_NB_state_sent;
    }

    if(exchange_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_transport_send(session, &exchange_state->c, 1, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send NEWKEYS message ECDH");
            goto clean_exit;
        }

        exchange_state->state = libssh2_NB_state_sent2;
    }

    if(exchange_state->state == libssh2_NB_state_sent2) {
        rc = _libssh2_packet_require(session, SSH_MSG_NEWKEYS,
                                     &exchange_state->tmp,
                                     &exchange_state->tmp_len, 0, NULL, 0,
                                     &exchange_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timed out waiting for NEWKEYS ECDH");
            goto clean_exit;
        }

        /* The first key exchange has been performed,
           switch to active crypt/comp/mac mode */
        session->state |= LIBSSH2_STATE_NEWKEYS;
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Received NEWKEYS message ECDH"));

        /* This will actually end up being just packet_type(1)
           for this packet type anyway */
        LIBSSH2_FREE(session, exchange_state->tmp);

        if(!session->session_id) {

            size_t digest_length = 0;

            if(type == LIBSSH2_EC_CURVE_NISTP256)
                digest_length = SHA256_DIGEST_LENGTH;
            else if(type == LIBSSH2_EC_CURVE_NISTP384)
                digest_length = SHA384_DIGEST_LENGTH;
            else if(type == LIBSSH2_EC_CURVE_NISTP521)
                digest_length = SHA512_DIGEST_LENGTH;
            else{
                ret = _libssh2_error(session, LIBSSH2_ERROR_KEX_FAILURE,
                                     "Unknown SHA digest for EC curve");
                goto clean_exit;

            }
            session->session_id = LIBSSH2_ALLOC(session, digest_length);
            if(!session->session_id) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                     "Unable to allocate buffer for "
                                     "SHA digest");
                goto clean_exit;
            }
            memcpy(session->session_id, exchange_state->h_sig_comp,
                   digest_length);
             session->session_id_len = (uint32_t)digest_length;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "session_id calculated"));
        }

        /* Cleanup any existing cipher */
        if(session->local.crypt->dtor) {
            session->local.crypt->dtor(session,
                                       &session->local.crypt_abstract);
        }

        /* Calculate IV/Secret/Key for each direction */
        if(session->local.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(iv,
                                                 session->local.crypt->
                                                 iv_len, "A");
            if(!iv) {
                ret = -1;
                goto clean_exit;
            }

            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(secret,
                                                 session->local.crypt->
                                                 secret_len, "C");

            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->local.crypt->
                init(session, session->local.crypt, iv, &free_iv, secret,
                     &free_secret, 1, &session->local.crypt_abstract)) {
                    LIBSSH2_FREE(session, iv);
                    LIBSSH2_FREE(session, secret);
                    ret = LIBSSH2_ERROR_KEX_FAILURE;
                    goto clean_exit;
                }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->local.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->local.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server IV and Key calculated"));

        if(session->remote.crypt->dtor) {
            /* Cleanup any existing cipher */
            session->remote.crypt->dtor(session,
                                        &session->remote.crypt_abstract);
        }

        if(session->remote.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(iv,
                                                 session->remote.crypt->
                                                 iv_len, "B");

            if(!iv) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(secret,
                                                 session->remote.crypt->
                                                 secret_len, "D");

            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->remote.crypt->
                init(session, session->remote.crypt, iv, &free_iv, secret,
                     &free_secret, 0, &session->remote.crypt_abstract)) {
                    LIBSSH2_FREE(session, iv);
                    LIBSSH2_FREE(session, secret);
                    ret = LIBSSH2_ERROR_KEX_FAILURE;
                    goto clean_exit;
                }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->remote.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->remote.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client IV and Key calculated"));

        if(session->local.mac->dtor) {
            session->local.mac->dtor(session, &session->local.mac_abstract);
        }

        if(session->local.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(key,
                                                 session->local.mac->
                                                 key_len, "E");

            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->local.mac->init(session, key, &free_key,
                                     &session->local.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->local.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server HMAC Key calculated"));

        if(session->remote.mac->dtor) {
            session->remote.mac->dtor(session, &session->remote.mac_abstract);
        }

        if(session->remote.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            LIBSSH2_KEX_METHOD_EC_SHA_VALUE_HASH(key,
                                                 session->remote.mac->
                                                 key_len, "F");

            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->remote.mac->init(session, key, &free_key,
                                      &session->remote.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->remote.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client HMAC Key calculated"));

        /* Initialize compression for each direction */

        /* Cleanup any existing compression */
        if(session->local.comp && session->local.comp->dtor) {
            session->local.comp->dtor(session, 1,
                                      &session->local.comp_abstract);
        }

        if(session->local.comp && session->local.comp->init) {
            if(session->local.comp->init(session, 1,
                                         &session->local.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server compression initialized"));

        if(session->remote.comp && session->remote.comp->dtor) {
            session->remote.comp->dtor(session, 0,
                                       &session->remote.comp_abstract);
        }

        if(session->remote.comp && session->remote.comp->init) {
            if(session->remote.comp->init(session, 0,
                                          &session->remote.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client compression initialized"));
    }

clean_exit:
    ecdh_exchange_state_cleanup(session, exchange_state);

    return ret;
}

/* kex_method_ecdh_key_exchange
 *
 * Elliptic Curve Diffie Hellman Key Exchange
 * supports SHA256/384/512 hashes based on negotiated ecdh method
 *
 */
static int
kex_method_ecdh_key_exchange
(LIBSSH2_SESSION * session, key_exchange_state_low_t * key_state)
{
    int ret = 0;
    int rc = 0;
    unsigned char *s;
    libssh2_curve_type type;

    if(key_state->state == libssh2_NB_state_idle) {

        key_state->public_key_oct = NULL;
        key_state->state = libssh2_NB_state_created;
    }

    if(key_state->state == libssh2_NB_state_created) {
        rc = kex_session_ecdh_curve_type(session->kex->name, &type);

        if(rc) {
            ret = _libssh2_error(session, -1,
                                 "Unknown KEX nistp curve type");
            goto ecdh_clean_exit;
        }

        rc = _libssh2_ecdsa_create_key(session, &key_state->private_key,
                                       &key_state->public_key_oct,
                                       &key_state->public_key_oct_len, type);

        if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to create private key");
            goto ecdh_clean_exit;
        }

        key_state->request[0] = SSH2_MSG_KEX_ECDH_INIT;
        s = key_state->request + 1;
        _libssh2_store_str(&s, (const char *)key_state->public_key_oct,
                           key_state->public_key_oct_len);
        key_state->request_len = key_state->public_key_oct_len + 5;

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating ECDH SHA2 NISTP256"));

        key_state->state = libssh2_NB_state_sent;
    }

    if(key_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_transport_send(session, key_state->request,
                                     key_state->request_len, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send ECDH_INIT");
            goto ecdh_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent1;
    }

    if(key_state->state == libssh2_NB_state_sent1) {
        rc = _libssh2_packet_require(session, SSH2_MSG_KEX_ECDH_REPLY,
                                     &key_state->data, &key_state->data_len,
                                     0, NULL, 0, &key_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timeout waiting for ECDH_REPLY reply");
            goto ecdh_clean_exit;
        }

        key_state->state = libssh2_NB_state_sent2;
    }

    if(key_state->state == libssh2_NB_state_sent2) {

        rc = kex_session_ecdh_curve_type(session->kex->name, &type);

        if(rc) {
            ret = _libssh2_error(session, -1,
                                 "Unknown KEX nistp curve type");
            goto ecdh_clean_exit;
        }

        ret = ecdh_sha2_nistp(session, type, key_state->data,
                              key_state->data_len,
                              (unsigned char *)key_state->public_key_oct,
                              key_state->public_key_oct_len,
                              key_state->private_key,
                              &key_state->exchange_state);

        if(ret == LIBSSH2_ERROR_EAGAIN) {
            return ret;
        }
    }

ecdh_clean_exit:

    kex_method_ecdh_cleanup(session, key_state);

    return ret;
}

#endif /* LIBSSH2_ECDSA */


#if LIBSSH2_ED25519

static void
curve25519_exchange_state_cleanup(LIBSSH2_SESSION * session,
                                  kmdhgGPshakex_state_t *exchange_state)
{
    _libssh2_bn_free(exchange_state->k);
    exchange_state->k = NULL;

    if(exchange_state->k_value) {
        LIBSSH2_FREE(session, exchange_state->k_value);
        exchange_state->k_value = NULL;
    }

    exchange_state->state = libssh2_NB_state_idle;
}

static void
kex_method_curve25519_cleanup
(LIBSSH2_SESSION * session, key_exchange_state_low_t * key_state)
{
    if(key_state->curve25519_public_key) {
        _libssh2_explicit_zero(key_state->curve25519_public_key,
                               LIBSSH2_ED25519_KEY_LEN);
        LIBSSH2_FREE(session, key_state->curve25519_public_key);
        key_state->curve25519_public_key = NULL;
    }

    if(key_state->curve25519_private_key) {
        _libssh2_explicit_zero(key_state->curve25519_private_key,
                               LIBSSH2_ED25519_KEY_LEN);
        LIBSSH2_FREE(session, key_state->curve25519_private_key);
        key_state->curve25519_private_key = NULL;
    }

    if(key_state->data) {
        LIBSSH2_FREE(session, key_state->data);
        key_state->data = NULL;
    }

    key_state->state = libssh2_NB_state_idle;

    if(key_state->exchange_state.state != libssh2_NB_state_idle) {
        curve25519_exchange_state_cleanup(session, &key_state->exchange_state);
    }
}

/* curve25519_sha256
 * Elliptic Curve Key Exchange
 */
static int
curve25519_sha256(LIBSSH2_SESSION *session, unsigned char *data,
                  size_t data_len,
                  unsigned char public_key[LIBSSH2_ED25519_KEY_LEN],
                  unsigned char private_key[LIBSSH2_ED25519_KEY_LEN],
                  kmdhgGPshakex_state_t *exchange_state)
{
    int ret = 0;
    int rc;
    int public_key_len = LIBSSH2_ED25519_KEY_LEN;

    if(data_len < 5) {
        return _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                              "Data is too short");
    }

    if(exchange_state->state == libssh2_NB_state_idle) {

        /* Setup initial values */
        exchange_state->k = _libssh2_bn_init();

        exchange_state->state = libssh2_NB_state_created;
    }

    if(exchange_state->state == libssh2_NB_state_created) {
        /* parse INIT reply data */
        unsigned char *server_public_key, *server_host_key;
        size_t server_public_key_len, hostkey_len;
        struct string_buf buf;

        if(data_len < 5) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected curve25519 key length 1");
            goto clean_exit;
        }

        buf.data = data;
        buf.len = data_len;
        buf.dataptr = buf.data;
        buf.dataptr++; /* advance past packet type */

        if(_libssh2_get_string(&buf, &server_host_key, &hostkey_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected curve25519 key length 2");
            goto clean_exit;
        }

        session->server_hostkey_len = (uint32_t)hostkey_len;
        session->server_hostkey = LIBSSH2_ALLOC(session,
                                                session->server_hostkey_len);
        if(!session->server_hostkey) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate memory for a copy "
                                 "of the host curve25519 key");
            goto clean_exit;
        }

        memcpy(session->server_hostkey, server_host_key,
               session->server_hostkey_len);

#if LIBSSH2_MD5
        {
            libssh2_md5_ctx fingerprint_ctx;

            if(libssh2_md5_init(&fingerprint_ctx) &&
               libssh2_md5_update(fingerprint_ctx, session->server_hostkey,
                                  session->server_hostkey_len) &&
               libssh2_md5_final(fingerprint_ctx,
                                 session->server_hostkey_md5)) {
                session->server_hostkey_md5_valid = TRUE;
            }
            else {
                session->server_hostkey_md5_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[50], *fprint = fingerprint;
            int i;
            for(i = 0; i < 16; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_md5[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's MD5 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */
#endif /* ! LIBSSH2_MD5 */

        {
            libssh2_sha1_ctx fingerprint_ctx;

            if(libssh2_sha1_init(&fingerprint_ctx) &&
               libssh2_sha1_update(fingerprint_ctx, session->server_hostkey,
                                   session->server_hostkey_len) &&
               libssh2_sha1_final(fingerprint_ctx,
                                  session->server_hostkey_sha1)) {
                session->server_hostkey_sha1_valid = TRUE;
            }
            else {
                session->server_hostkey_sha1_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char fingerprint[64], *fprint = fingerprint;
            int i;
            for(i = 0; i < 20; i++, fprint += 3) {
                snprintf(fprint, 4, "%02x:", session->server_hostkey_sha1[i]);
            }
            *(--fprint) = '\0';
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Server's SHA1 Fingerprint: %s", fingerprint));
        }
#endif /* LIBSSH2DEBUG */

        /* SHA256 */
        {
            libssh2_sha256_ctx fingerprint_ctx;

            if(libssh2_sha256_init(&fingerprint_ctx) &&
               libssh2_sha256_update(fingerprint_ctx, session->server_hostkey,
                                     session->server_hostkey_len) &&
               libssh2_sha256_final(fingerprint_ctx,
                                    session->server_hostkey_sha256)) {
                session->server_hostkey_sha256_valid = TRUE;
            }
            else {
                session->server_hostkey_sha256_valid = FALSE;
            }
        }
#ifdef LIBSSH2DEBUG
        {
            char *base64Fingerprint = NULL;
            _libssh2_base64_encode(session,
                                   (const char *)
                                   session->server_hostkey_sha256,
                                   SHA256_DIGEST_LENGTH, &base64Fingerprint);
            if(base64Fingerprint) {
                _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                               "Server's SHA256 Fingerprint: %s",
                               base64Fingerprint));
                LIBSSH2_FREE(session, base64Fingerprint);
            }
        }
#endif /* LIBSSH2DEBUG */

        if(session->hostkey->init(session, session->server_hostkey,
                                  session->server_hostkey_len,
                                  &session->server_hostkey_abstract)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unable to initialize hostkey importer "
                                 "curve25519");
            goto clean_exit;
        }

        /* server public key Q_S */
        if(_libssh2_get_string(&buf, &server_public_key,
                               &server_public_key_len)) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Unexpected curve25519 key length");
            goto clean_exit;
        }

        if(server_public_key_len != LIBSSH2_ED25519_KEY_LEN) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unexpected curve25519 server "
                                 "public key length");
            goto clean_exit;
        }

        /* server signature */
        if(_libssh2_get_string(&buf, &exchange_state->h_sig,
           &(exchange_state->h_sig_len))) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_INIT,
                                 "Unexpected curve25519 server sig length");
            goto clean_exit;
        }

        /* Compute the shared secret K */
        rc = _libssh2_curve25519_gen_k(&exchange_state->k, private_key,
                                       server_public_key);
        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_KEX_FAILURE,
                                 "Unable to create curve25519 shared secret");
            goto clean_exit;
        }

        exchange_state->k_value_len = _libssh2_bn_bytes(exchange_state->k) + 5;
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            /* don't need leading 00 */
            exchange_state->k_value_len--;
        }
        exchange_state->k_value =
        LIBSSH2_ALLOC(session, exchange_state->k_value_len);
        if(!exchange_state->k_value) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate buffer for K");
            goto clean_exit;
        }
        _libssh2_htonu32(exchange_state->k_value,
                         (uint32_t)(exchange_state->k_value_len - 4));
        if(_libssh2_bn_bits(exchange_state->k) % 8) {
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 4)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->e");
                goto clean_exit;
            }
        }
        else {
            exchange_state->k_value[4] = 0;
            if(_libssh2_bn_to_bin(exchange_state->k,
                                  exchange_state->k_value + 5)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_OUT_OF_BOUNDARY,
                                     "Can't write exchange_state->e");
                goto clean_exit;
            }
        }

        /*/ verify hash */
        LIBSSH2_KEX_METHOD_EC_SHA_HASH_CREATE_VERIFY(256);

        if(rc) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_HOSTKEY_SIGN,
                                 "Unable to verify hostkey signature "
                                 "curve25519");
            goto clean_exit;
        }

        exchange_state->c = SSH_MSG_NEWKEYS;
        exchange_state->state = libssh2_NB_state_sent;
    }

    if(exchange_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_transport_send(session, &exchange_state->c, 1, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send NEWKEYS message curve25519");
            goto clean_exit;
        }

        exchange_state->state = libssh2_NB_state_sent2;
    }

    if(exchange_state->state == libssh2_NB_state_sent2) {
        rc = _libssh2_packet_require(session, SSH_MSG_NEWKEYS,
                                     &exchange_state->tmp,
                                     &exchange_state->tmp_len, 0, NULL, 0,
                                     &exchange_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timed out waiting for NEWKEYS curve25519");
            goto clean_exit;
        }

        /* The first key exchange has been performed,
           switch to active crypt/comp/mac mode */
        session->state |= LIBSSH2_STATE_NEWKEYS;
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Received NEWKEYS message curve25519"));

        /* This will actually end up being just packet_type(1)
           for this packet type anyway */
        LIBSSH2_FREE(session, exchange_state->tmp);

        if(!session->session_id) {

            size_t digest_length = SHA256_DIGEST_LENGTH;
            session->session_id = LIBSSH2_ALLOC(session, digest_length);
            if(!session->session_id) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                     "Unable to allocate buffer for "
                                     "SHA digest");
                goto clean_exit;
            }
            memcpy(session->session_id, exchange_state->h_sig_comp,
                   digest_length);
            session->session_id_len = (uint32_t)digest_length;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "session_id calculated"));
        }

        /* Cleanup any existing cipher */
        if(session->local.crypt->dtor) {
            session->local.crypt->dtor(session,
                                       &session->local.crypt_abstract);
        }

        /* Calculate IV/Secret/Key for each direction */
        if(session->local.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, iv,
                                              session->local.crypt->
                                              iv_len, "A");
            if(!iv) {
                ret = -1;
                goto clean_exit;
            }

            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, secret,
                                              session->local.crypt->
                                              secret_len, "C");

            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->local.crypt->
                init(session, session->local.crypt, iv, &free_iv, secret,
                     &free_secret, 1, &session->local.crypt_abstract)) {
                    LIBSSH2_FREE(session, iv);
                    LIBSSH2_FREE(session, secret);
                    ret = LIBSSH2_ERROR_KEX_FAILURE;
                    goto clean_exit;
                }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->local.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->local.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server IV and Key calculated"));

        if(session->remote.crypt->dtor) {
            /* Cleanup any existing cipher */
            session->remote.crypt->dtor(session,
                                        &session->remote.crypt_abstract);
        }

        if(session->remote.crypt->init) {
            unsigned char *iv = NULL, *secret = NULL;
            int free_iv = 0, free_secret = 0;

            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, iv,
                                              session->remote.crypt->
                                              iv_len, "B");

            if(!iv) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, secret,
                                              session->remote.crypt->
                                              secret_len, "D");

            if(!secret) {
                LIBSSH2_FREE(session, iv);
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            if(session->remote.crypt->
                init(session, session->remote.crypt, iv, &free_iv, secret,
                     &free_secret, 0, &session->remote.crypt_abstract)) {
                    LIBSSH2_FREE(session, iv);
                    LIBSSH2_FREE(session, secret);
                    ret = LIBSSH2_ERROR_KEX_FAILURE;
                    goto clean_exit;
                }

            if(free_iv) {
                _libssh2_explicit_zero(iv, session->remote.crypt->iv_len);
                LIBSSH2_FREE(session, iv);
            }

            if(free_secret) {
                _libssh2_explicit_zero(secret,
                                       session->remote.crypt->secret_len);
                LIBSSH2_FREE(session, secret);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client IV and Key calculated"));

        if(session->local.mac->dtor) {
            session->local.mac->dtor(session, &session->local.mac_abstract);
        }

        if(session->local.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, key,
                                              session->local.mac->
                                              key_len, "E");

            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->local.mac->init(session, key, &free_key,
                                     &session->local.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->local.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server HMAC Key calculated"));

        if(session->remote.mac->dtor) {
            session->remote.mac->dtor(session, &session->remote.mac_abstract);
        }

        if(session->remote.mac->init) {
            unsigned char *key = NULL;
            int free_key = 0;

            LIBSSH2_KEX_METHOD_SHA_VALUE_HASH(256, key,
                                              session->remote.mac->
                                              key_len, "F");

            if(!key) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
            session->remote.mac->init(session, key, &free_key,
                                      &session->remote.mac_abstract);

            if(free_key) {
                _libssh2_explicit_zero(key, session->remote.mac->key_len);
                LIBSSH2_FREE(session, key);
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client HMAC Key calculated"));

        /* Initialize compression for each direction */

        /* Cleanup any existing compression */
        if(session->local.comp && session->local.comp->dtor) {
            session->local.comp->dtor(session, 1,
                                      &session->local.comp_abstract);
        }

        if(session->local.comp && session->local.comp->init) {
            if(session->local.comp->init(session, 1,
                                         &session->local.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Client to Server compression initialized"));

        if(session->remote.comp && session->remote.comp->dtor) {
            session->remote.comp->dtor(session, 0,
                                       &session->remote.comp_abstract);
        }

        if(session->remote.comp && session->remote.comp->init) {
            if(session->remote.comp->init(session, 0,
                                          &session->remote.comp_abstract)) {
                ret = LIBSSH2_ERROR_KEX_FAILURE;
                goto clean_exit;
            }
        }
        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Server to Client compression initialized"));
    }

clean_exit:
    curve25519_exchange_state_cleanup(session, exchange_state);

    return ret;
}

/* kex_method_curve25519_key_exchange
 *
 * Elliptic Curve X25519 Key Exchange with SHA256 hash
 *
 */
static int
kex_method_curve25519_key_exchange
(LIBSSH2_SESSION * session, key_exchange_state_low_t * key_state)
{
    int ret = 0;
    int rc = 0;

    if(key_state->state == libssh2_NB_state_idle) {

        key_state->public_key_oct = NULL;
        key_state->state = libssh2_NB_state_created;
    }

    if(key_state->state == libssh2_NB_state_created) {
        unsigned char *s = NULL;

        rc = strcmp(session->kex->name, "curve25519-sha256@libssh.org");
        if(rc)
            rc = strcmp(session->kex->name, "curve25519-sha256");

        if(rc) {
            ret = _libssh2_error(session, -1,
                                 "Unknown KEX curve25519 curve type");
            goto clean_exit;
        }

        rc = _libssh2_curve25519_new(session,
                                     &key_state->curve25519_public_key,
                                     &key_state->curve25519_private_key);

        if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to create private key");
            goto clean_exit;
        }

        key_state->request[0] = SSH2_MSG_KEX_ECDH_INIT;
        s = key_state->request + 1;
        _libssh2_store_str(&s, (const char *)key_state->curve25519_public_key,
                           LIBSSH2_ED25519_KEY_LEN);
        key_state->request_len = LIBSSH2_ED25519_KEY_LEN + 5;

        _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                       "Initiating curve25519 SHA2"));

        key_state->state = libssh2_NB_state_sent;
    }

    if(key_state->state == libssh2_NB_state_sent) {
        rc = _libssh2_transport_send(session, key_state->request,
                                     key_state->request_len, NULL, 0);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Unable to send ECDH_INIT");
            goto clean_exit;
        }

        key_state->state = libssh2_NB_state_sent1;
    }

    if(key_state->state == libssh2_NB_state_sent1) {
        rc = _libssh2_packet_require(session, SSH2_MSG_KEX_ECDH_REPLY,
                                     &key_state->data, &key_state->data_len,
                                     0, NULL, 0, &key_state->req_state);
        if(rc == LIBSSH2_ERROR_EAGAIN) {
            return rc;
        }
        else if(rc) {
            ret = _libssh2_error(session, rc,
                                 "Timeout waiting for ECDH_REPLY reply");
            goto clean_exit;
        }

        key_state->state = libssh2_NB_state_sent2;
    }

    if(key_state->state == libssh2_NB_state_sent2) {

        ret = curve25519_sha256(session, key_state->data, key_state->data_len,
                                key_state->curve25519_public_key,
                                key_state->curve25519_private_key,
                                &key_state->exchange_state);

        if(ret == LIBSSH2_ERROR_EAGAIN) {
            return ret;
        }
    }

clean_exit:

    kex_method_curve25519_cleanup(session, key_state);

    return ret;
}


#endif /* LIBSSH2_ED25519 */


#define LIBSSH2_KEX_METHOD_FLAG_REQ_ENC_HOSTKEY     0x0001
#define LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY    0x0002

static const LIBSSH2_KEX_METHOD kex_method_diffie_helman_group1_sha1 = {
    "diffie-hellman-group1-sha1",
    kex_method_diffie_hellman_group1_sha1_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD kex_method_diffie_helman_group14_sha1 = {
    "diffie-hellman-group14-sha1",
    kex_method_diffie_hellman_group14_sha1_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD kex_method_diffie_helman_group14_sha256 = {
    "diffie-hellman-group14-sha256",
    kex_method_diffie_hellman_group14_sha256_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD kex_method_diffie_helman_group16_sha512 = {
    "diffie-hellman-group16-sha512",
    kex_method_diffie_hellman_group16_sha512_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD kex_method_diffie_helman_group18_sha512 = {
    "diffie-hellman-group18-sha512",
    kex_method_diffie_hellman_group18_sha512_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD
kex_method_diffie_helman_group_exchange_sha1 = {
    "diffie-hellman-group-exchange-sha1",
    kex_method_diffie_hellman_group_exchange_sha1_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD
kex_method_diffie_helman_group_exchange_sha256 = {
    "diffie-hellman-group-exchange-sha256",
    kex_method_diffie_hellman_group_exchange_sha256_key_exchange,
    kex_diffie_hellman_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

#if LIBSSH2_ECDSA
static const LIBSSH2_KEX_METHOD
kex_method_ecdh_sha2_nistp256 = {
    "ecdh-sha2-nistp256",
    kex_method_ecdh_key_exchange,
    kex_method_ecdh_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD
kex_method_ecdh_sha2_nistp384 = {
    "ecdh-sha2-nistp384",
    kex_method_ecdh_key_exchange,
    kex_method_ecdh_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};

static const LIBSSH2_KEX_METHOD
kex_method_ecdh_sha2_nistp521 = {
    "ecdh-sha2-nistp521",
    kex_method_ecdh_key_exchange,
    kex_method_ecdh_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};
#endif

#if LIBSSH2_ED25519
static const LIBSSH2_KEX_METHOD
kex_method_ssh_curve25519_sha256_libssh = {
    "curve25519-sha256@libssh.org",
    kex_method_curve25519_key_exchange,
    kex_method_curve25519_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};
static const LIBSSH2_KEX_METHOD
kex_method_ssh_curve25519_sha256 = {
    "curve25519-sha256",
    kex_method_curve25519_key_exchange,
    kex_method_curve25519_cleanup,
    LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY,
};
#endif

/* this kex method signals that client can receive extensions
 * as described in https://datatracker.ietf.org/doc/html/rfc8308
*/

static const LIBSSH2_KEX_METHOD
kex_method_extension_negotiation = {
    "ext-info-c",
    NULL,
    NULL,
    0,
};

static const LIBSSH2_KEX_METHOD
kex_method_strict_client_extension = {
    "kex-strict-c-v00@openssh.com",
    NULL,
    NULL,
    0,
};

static const LIBSSH2_KEX_METHOD *libssh2_kex_methods[] = {
#if LIBSSH2_ED25519
    &kex_method_ssh_curve25519_sha256,
    &kex_method_ssh_curve25519_sha256_libssh,
#endif
#if LIBSSH2_ECDSA
    &kex_method_ecdh_sha2_nistp256,
    &kex_method_ecdh_sha2_nistp384,
    &kex_method_ecdh_sha2_nistp521,
#endif
    &kex_method_diffie_helman_group_exchange_sha256,
    &kex_method_diffie_helman_group16_sha512,
    &kex_method_diffie_helman_group18_sha512,
    &kex_method_diffie_helman_group14_sha256,
    &kex_method_diffie_helman_group14_sha1,
    &kex_method_diffie_helman_group1_sha1,
    &kex_method_diffie_helman_group_exchange_sha1,
    &kex_method_extension_negotiation,
    &kex_method_strict_client_extension,
    NULL
};

typedef struct _LIBSSH2_COMMON_METHOD
{
    const char *name;
} LIBSSH2_COMMON_METHOD;

/* kex_method_strlen
 *
 * Calculate the length of a particular method list's resulting string
 * Includes SUM(strlen() of each individual method plus 1 (for coma)) - 1
 * (because the last coma isn't used)
 * Another sign of bad coding practices gone mad.  Pretend you don't see this.
 */
static size_t
kex_method_strlen(LIBSSH2_COMMON_METHOD ** method)
{
    size_t len = 0;

    if(!method || !*method) {
        return 0;
    }

    while(*method && (*method)->name) {
        len += strlen((*method)->name) + 1;
        method++;
    }

    return len - 1;
}



/* kex_method_list
 * Generate formatted preference list in buf
 */
static uint32_t
kex_method_list(unsigned char *buf, uint32_t list_strlen,
                LIBSSH2_COMMON_METHOD ** method)
{
    _libssh2_htonu32(buf, list_strlen);
    buf += 4;

    if(!method || !*method) {
        return 4;
    }

    while(*method && (*method)->name) {
        uint32_t mlen = (uint32_t)strlen((*method)->name);
        memcpy(buf, (*method)->name, mlen);
        buf += mlen;
        *(buf++) = ',';
        method++;
    }

    return list_strlen + 4;
}



#define LIBSSH2_METHOD_PREFS_LEN(prefvar, defaultvar)           \
    (uint32_t)((prefvar) ? strlen(prefvar) :                    \
        kex_method_strlen((LIBSSH2_COMMON_METHOD**)(defaultvar)))

#define LIBSSH2_METHOD_PREFS_STR(buf, prefvarlen, prefvar, defaultvar)     \
    do {                                                                   \
        if(prefvar) {                                                      \
            _libssh2_htonu32((buf), (prefvarlen));                         \
            buf += 4;                                                      \
            memcpy((buf), (prefvar), (prefvarlen));                        \
            buf += (prefvarlen);                                           \
        }                                                                  \
        else {                                                             \
            buf += kex_method_list((buf), (prefvarlen),                    \
                                   (LIBSSH2_COMMON_METHOD**)(defaultvar)); \
        }                                                                  \
    } while(0)

/* kexinit
 * Send SSH_MSG_KEXINIT packet
 */
static int kexinit(LIBSSH2_SESSION * session)
{
    /* 62 = packet_type(1) + cookie(16) + first_packet_follows(1) +
       reserved(4) + length longs(40) */
    size_t data_len = 62;
    unsigned char *data, *s;
    int rc;

    if(session->kexinit_state == libssh2_NB_state_idle) {
        uint32_t kex_len, hostkey_len;
        uint32_t crypt_cs_len, crypt_sc_len;
        uint32_t comp_cs_len, comp_sc_len;
        uint32_t mac_cs_len, mac_sc_len;
        uint32_t lang_cs_len, lang_sc_len;

        kex_len =
            LIBSSH2_METHOD_PREFS_LEN(session->kex_prefs, libssh2_kex_methods);
        hostkey_len =
            LIBSSH2_METHOD_PREFS_LEN(session->hostkey_prefs,
                                     libssh2_hostkey_methods());
        crypt_cs_len =
            LIBSSH2_METHOD_PREFS_LEN(session->local.crypt_prefs,
                                     libssh2_crypt_methods());
        crypt_sc_len =
            LIBSSH2_METHOD_PREFS_LEN(session->remote.crypt_prefs,
                                     libssh2_crypt_methods());
        mac_cs_len =
            LIBSSH2_METHOD_PREFS_LEN(session->local.mac_prefs,
                                     _libssh2_mac_methods());
        mac_sc_len =
            LIBSSH2_METHOD_PREFS_LEN(session->remote.mac_prefs,
                                     _libssh2_mac_methods());
        comp_cs_len =
            LIBSSH2_METHOD_PREFS_LEN(session->local.comp_prefs,
                                     _libssh2_comp_methods(session));
        comp_sc_len =
            LIBSSH2_METHOD_PREFS_LEN(session->remote.comp_prefs,
                                     _libssh2_comp_methods(session));
        lang_cs_len =
            LIBSSH2_METHOD_PREFS_LEN(session->local.lang_prefs, NULL);
        lang_sc_len =
            LIBSSH2_METHOD_PREFS_LEN(session->remote.lang_prefs, NULL);

        data_len += kex_len + hostkey_len + crypt_cs_len + crypt_sc_len +
                    comp_cs_len + comp_sc_len + mac_cs_len + mac_sc_len +
                    lang_cs_len + lang_sc_len;

        s = data = LIBSSH2_ALLOC(session, data_len);
        if(!data) {
            return _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                  "Unable to allocate memory");
        }

        *(s++) = SSH_MSG_KEXINIT;

        if(_libssh2_random(s, 16)) {
            return _libssh2_error(session, LIBSSH2_ERROR_RANDGEN,
                                  "Unable to get random bytes "
                                  "for KEXINIT cookie");
        }
        s += 16;

        /* Ennumerating through these lists twice is probably (certainly?)
           inefficient from a CPU standpoint, but it saves multiple
           malloc/realloc calls */
        LIBSSH2_METHOD_PREFS_STR(s, kex_len, session->kex_prefs,
                                 libssh2_kex_methods);
        LIBSSH2_METHOD_PREFS_STR(s, hostkey_len, session->hostkey_prefs,
                                 libssh2_hostkey_methods());
        LIBSSH2_METHOD_PREFS_STR(s, crypt_cs_len, session->local.crypt_prefs,
                                 libssh2_crypt_methods());
        LIBSSH2_METHOD_PREFS_STR(s, crypt_sc_len, session->remote.crypt_prefs,
                                 libssh2_crypt_methods());
        LIBSSH2_METHOD_PREFS_STR(s, mac_cs_len, session->local.mac_prefs,
                                 _libssh2_mac_methods());
        LIBSSH2_METHOD_PREFS_STR(s, mac_sc_len, session->remote.mac_prefs,
                                 _libssh2_mac_methods());
        LIBSSH2_METHOD_PREFS_STR(s, comp_cs_len, session->local.comp_prefs,
                                 _libssh2_comp_methods(session));
        LIBSSH2_METHOD_PREFS_STR(s, comp_sc_len, session->remote.comp_prefs,
                                 _libssh2_comp_methods(session));
        LIBSSH2_METHOD_PREFS_STR(s, lang_cs_len, session->local.lang_prefs,
                                 NULL);
        LIBSSH2_METHOD_PREFS_STR(s, lang_sc_len, session->remote.lang_prefs,
                                 NULL);

        /* No optimistic KEX packet follows */
        /* Deal with optimistic packets
         * session->flags |= KEXINIT_OPTIMISTIC
         * session->flags |= KEXINIT_METHODSMATCH
         */
        *(s++) = 0;

        /* Reserved == 0 */
        _libssh2_htonu32(s, 0);

#ifdef LIBSSH2DEBUG
        {
            /* Funnily enough, they'll all "appear" to be '\0' terminated */
            unsigned char *p = data + 21;   /* type(1) + cookie(16) + len(4) */

            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent KEX: %s", p));
            p += kex_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent HOSTKEY: %s", p));
            p += hostkey_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent CRYPT_CS: %s", p));
            p += crypt_cs_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent CRYPT_SC: %s", p));
            p += crypt_sc_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent MAC_CS: %s", p));
            p += mac_cs_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent MAC_SC: %s", p));
            p += mac_sc_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent COMP_CS: %s", p));
            p += comp_cs_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent COMP_SC: %s", p));
            p += comp_sc_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent LANG_CS: %s", p));
            p += lang_cs_len + 4;
            _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                           "Sent LANG_SC: %s", p));
            p += lang_sc_len + 4;
        }
#endif /* LIBSSH2DEBUG */

        session->kexinit_state = libssh2_NB_state_created;
    }
    else {
        data = session->kexinit_data;
        data_len = session->kexinit_data_len;
        /* zap the variables to ensure there is NOT a double free later */
        session->kexinit_data = NULL;
        session->kexinit_data_len = 0;
    }

    rc = _libssh2_transport_send(session, data, data_len, NULL, 0);
    if(rc == LIBSSH2_ERROR_EAGAIN) {
        session->kexinit_data = data;
        session->kexinit_data_len = data_len;
        return rc;
    }
    else if(rc) {
        LIBSSH2_FREE(session, data);
        session->kexinit_state = libssh2_NB_state_idle;
        return _libssh2_error(session, rc,
                              "Unable to send KEXINIT packet to remote host");

    }

    if(session->local.kexinit) {
        LIBSSH2_FREE(session, session->local.kexinit);
    }

    session->local.kexinit = data;
    session->local.kexinit_len = data_len;

    session->kexinit_state = libssh2_NB_state_idle;

    return 0;
}

/* _libssh2_kex_agree_instr
 * Kex specific variant of strstr()
 * Needle must be preceded by BOL or ',', and followed by ',' or EOL
 */
unsigned char *
_libssh2_kex_agree_instr(unsigned char *haystack, size_t haystack_len,
                         const unsigned char *needle, size_t needle_len)
{
    unsigned char *s;
    unsigned char *end_haystack;
    size_t left;

    if(!haystack || !needle) {
        return NULL;
    }

    /* Haystack too short to bother trying */
    if(haystack_len < needle_len || needle_len == 0) {
        return NULL;
    }

    s = haystack;
    end_haystack = &haystack[haystack_len];
    left = end_haystack - s;

    /* Needle at start of haystack */
    if((strncmp((char *) haystack, (char *) needle, needle_len) == 0) &&
        (needle_len == haystack_len || haystack[needle_len] == ',')) {
        return haystack;
    }

    /* Search until we run out of comas or we run out of haystack,
       whichever comes first */
    /* !checksrc! disable EQUALSNULL 1 */
    while((s = (unsigned char *) memchr((char *) s, ',', left)) != NULL) {
        /* Advance buffer past coma if we can */
        left = end_haystack - s;
        if((left >= 1) && (left <= haystack_len) && (left > needle_len)) {
            s++;
            left--;
        }
        else {
            return NULL;
        }

        /* Needle at X position */
        if((strncmp((char *) s, (char *) needle, needle_len) == 0) &&
            (((s - haystack) + needle_len) == haystack_len
             || s[needle_len] == ',')) {
            return s;
        }
    }

    return NULL;
}



/* kex_get_method_by_name
 */
static const LIBSSH2_COMMON_METHOD *
kex_get_method_by_name(const char *name, size_t name_len,
                       const LIBSSH2_COMMON_METHOD ** methodlist)
{
    while(*methodlist) {
        if((strlen((*methodlist)->name) == name_len) &&
            (strncmp((*methodlist)->name, name, name_len) == 0)) {
            return *methodlist;
        }
        methodlist++;
    }
    return NULL;
}



/* kex_agree_hostkey
 * Agree on a Hostkey which works with this kex
 */
static int kex_agree_hostkey(LIBSSH2_SESSION * session,
                             size_t kex_flags,
                             unsigned char *hostkey, size_t hostkey_len)
{
    const LIBSSH2_HOSTKEY_METHOD **hostkeyp = libssh2_hostkey_methods();
    unsigned char *s;

    if(session->hostkey_prefs) {
        s = (unsigned char *) session->hostkey_prefs;

        while(s && *s) {
            unsigned char *p = (unsigned char *) strchr((char *) s, ',');
            size_t method_len = (p ? (size_t)(p - s) : strlen((char *) s));
            if(_libssh2_kex_agree_instr(hostkey, hostkey_len, s, method_len)) {
                const LIBSSH2_HOSTKEY_METHOD *method =
                    (const LIBSSH2_HOSTKEY_METHOD *)
                    kex_get_method_by_name((char *) s, method_len,
                                           (const LIBSSH2_COMMON_METHOD **)
                                           hostkeyp);

                if(!method) {
                    /* Invalid method -- Should never be reached */
                    return -1;
                }

                /* So far so good, but does it suit our purposes? (Encrypting
                   vs Signing) */
                if(((kex_flags & LIBSSH2_KEX_METHOD_FLAG_REQ_ENC_HOSTKEY) ==
                     0) || (method->encrypt)) {
                    /* Either this hostkey can do encryption or this kex just
                       doesn't require it */
                    if(((kex_flags & LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY)
                         == 0) || (method->sig_verify)) {
                        /* Either this hostkey can do signing or this kex just
                           doesn't require it */
                        session->hostkey = method;
                        return 0;
                    }
                }
            }

            s = p ? p + 1 : NULL;
        }
        return -1;
    }

    while(hostkeyp && (*hostkeyp) && (*hostkeyp)->name) {
        s = _libssh2_kex_agree_instr(hostkey, hostkey_len,
                                     (unsigned char *) (*hostkeyp)->name,
                                     strlen((*hostkeyp)->name));
        if(s) {
            /* So far so good, but does it suit our purposes? (Encrypting vs
               Signing) */
            if(((kex_flags & LIBSSH2_KEX_METHOD_FLAG_REQ_ENC_HOSTKEY) == 0) ||
                ((*hostkeyp)->encrypt)) {
                /* Either this hostkey can do encryption or this kex just
                   doesn't require it */
                if(((kex_flags & LIBSSH2_KEX_METHOD_FLAG_REQ_SIGN_HOSTKEY) ==
                     0) || ((*hostkeyp)->sig_verify)) {
                    /* Either this hostkey can do signing or this kex just
                       doesn't require it */
                    session->hostkey = *hostkeyp;
                    return 0;
                }
            }
        }
        hostkeyp++;
    }

    return -1;
}



/* kex_agree_kex_hostkey
 * Agree on a Key Exchange method and a hostkey encoding type
 */
static int kex_agree_kex_hostkey(LIBSSH2_SESSION * session, unsigned char *kex,
                                 size_t kex_len, unsigned char *hostkey,
                                 size_t hostkey_len)
{
    const LIBSSH2_KEX_METHOD **kexp = libssh2_kex_methods;
    unsigned char *s;
    const unsigned char *strict =
        (unsigned char *)"kex-strict-s-v00@openssh.com";

    if(_libssh2_kex_agree_instr(kex, kex_len, strict, 28)) {
        session->kex_strict = 1;
    }

    if(session->kex_prefs) {
        s = (unsigned char *) session->kex_prefs;

        while(s && *s) {
            unsigned char *q, *p = (unsigned char *) strchr((char *) s, ',');
            size_t method_len = (p ? (size_t)(p - s) : strlen((char *) s));
            q = _libssh2_kex_agree_instr(kex, kex_len, s, method_len);
            if(q) {
                const LIBSSH2_KEX_METHOD *method = (const LIBSSH2_KEX_METHOD *)
                    kex_get_method_by_name((char *) s, method_len,
                                           (const LIBSSH2_COMMON_METHOD **)
                                           kexp);

                if(!method) {
                    /* Invalid method -- Should never be reached */
                    return -1;
                }

                /* We've agreed on a key exchange method,
                 * Can we agree on a hostkey that works with this kex?
                 */
                if(kex_agree_hostkey(session, method->flags, hostkey,
                                     hostkey_len) == 0) {
                    session->kex = method;
                    if(session->burn_optimistic_kexinit && (kex == q)) {
                        /* Server sent an optimistic packet, and client agrees
                         * with preference cancel burning the first KEX_INIT
                         * packet that comes in */
                        session->burn_optimistic_kexinit = 0;
                    }
                    return 0;
                }
            }

            s = p ? p + 1 : NULL;
        }
        return -1;
    }

    while(*kexp && (*kexp)->name) {
        s = _libssh2_kex_agree_instr(kex, kex_len,
                                     (unsigned char *) (*kexp)->name,
                                     strlen((*kexp)->name));
        if(s) {
            /* We've agreed on a key exchange method,
             * Can we agree on a hostkey that works with this kex?
             */
            if(kex_agree_hostkey(session, (*kexp)->flags, hostkey,
                                 hostkey_len) == 0) {
                session->kex = *kexp;
                if(session->burn_optimistic_kexinit && (kex == s)) {
                    /* Server sent an optimistic packet, and client agrees
                     * with preference cancel burning the first KEX_INIT
                     * packet that comes in */
                    session->burn_optimistic_kexinit = 0;
                }
                return 0;
            }
        }
        kexp++;
    }
    return -1;
}



/* kex_agree_crypt
 * Agree on a cipher algo
 */
static int kex_agree_crypt(LIBSSH2_SESSION * session,
                           libssh2_endpoint_data *endpoint,
                           unsigned char *crypt,
                           size_t crypt_len)
{
    const LIBSSH2_CRYPT_METHOD **cryptp = libssh2_crypt_methods();
    unsigned char *s;

    (void)session;

    if(endpoint->crypt_prefs) {
        s = (unsigned char *) endpoint->crypt_prefs;

        while(s && *s) {
            unsigned char *p = (unsigned char *) strchr((char *) s, ',');
            size_t method_len = (p ? (size_t)(p - s) : strlen((char *) s));

            if(_libssh2_kex_agree_instr(crypt, crypt_len, s, method_len)) {
                const LIBSSH2_CRYPT_METHOD *method =
                    (const LIBSSH2_CRYPT_METHOD *)
                    kex_get_method_by_name((char *) s, method_len,
                                           (const LIBSSH2_COMMON_METHOD **)
                                           cryptp);

                if(!method) {
                    /* Invalid method -- Should never be reached */
                    return -1;
                }

                endpoint->crypt = method;
                return 0;
            }

            s = p ? p + 1 : NULL;
        }
        return -1;
    }

    while(*cryptp && (*cryptp)->name) {
        s = _libssh2_kex_agree_instr(crypt, crypt_len,
                                     (unsigned char *) (*cryptp)->name,
                                     strlen((*cryptp)->name));
        if(s) {
            endpoint->crypt = *cryptp;
            return 0;
        }
        cryptp++;
    }

    return -1;
}



/* kex_agree_mac
 * Agree on a message authentication hash
 */
static int kex_agree_mac(LIBSSH2_SESSION * session,
                         libssh2_endpoint_data * endpoint, unsigned char *mac,
                         size_t mac_len)
{
    const LIBSSH2_MAC_METHOD **macp = _libssh2_mac_methods();
    const LIBSSH2_MAC_METHOD *override;
    unsigned char *s;
    (void)session;

    override = _libssh2_mac_override(endpoint->crypt);
    if(override) {
        /* This crypto method has its own hmac method built-in, so a separate
         * negotiation (and use) of a separate hmac method is unnecessary */
        endpoint->mac = override;
        return 0;
    }

    if(endpoint->mac_prefs) {
        s = (unsigned char *) endpoint->mac_prefs;

        while(s && *s) {
            unsigned char *p = (unsigned char *) strchr((char *) s, ',');
            size_t method_len = (p ? (size_t)(p - s) : strlen((char *) s));

            if(_libssh2_kex_agree_instr(mac, mac_len, s, method_len)) {
                const LIBSSH2_MAC_METHOD *method = (const LIBSSH2_MAC_METHOD *)
                    kex_get_method_by_name((char *) s, method_len,
                                           (const LIBSSH2_COMMON_METHOD **)
                                           macp);

                if(!method) {
                    /* Invalid method -- Should never be reached */
                    return -1;
                }

                endpoint->mac = method;
                return 0;
            }

            s = p ? p + 1 : NULL;
        }
        return -1;
    }

    while(*macp && (*macp)->name) {
        s = _libssh2_kex_agree_instr(mac, mac_len,
                                     (unsigned char *) (*macp)->name,
                                     strlen((*macp)->name));
        if(s) {
            endpoint->mac = *macp;
            return 0;
        }
        macp++;
    }

    return -1;
}



/* kex_agree_comp
 * Agree on a compression scheme
 */
static int kex_agree_comp(LIBSSH2_SESSION *session,
                          libssh2_endpoint_data *endpoint, unsigned char *comp,
                          size_t comp_len)
{
    const LIBSSH2_COMP_METHOD **compp = _libssh2_comp_methods(session);
    unsigned char *s;
    (void)session;

    if(endpoint->comp_prefs) {
        s = (unsigned char *) endpoint->comp_prefs;

        while(s && *s) {
            unsigned char *p = (unsigned char *) strchr((char *) s, ',');
            size_t method_len = (p ? (size_t)(p - s) : strlen((char *) s));

            if(_libssh2_kex_agree_instr(comp, comp_len, s, method_len)) {
                const LIBSSH2_COMP_METHOD *method =
                    (const LIBSSH2_COMP_METHOD *)
                    kex_get_method_by_name((char *) s, method_len,
                                           (const LIBSSH2_COMMON_METHOD **)
                                           compp);

                if(!method) {
                    /* Invalid method -- Should never be reached */
                    return -1;
                }

                endpoint->comp = method;
                return 0;
            }

            s = p ? p + 1 : NULL;
        }
        return -1;
    }

    while(*compp && (*compp)->name) {
        s = _libssh2_kex_agree_instr(comp, comp_len,
                                     (unsigned char *) (*compp)->name,
                                     strlen((*compp)->name));
        if(s) {
            endpoint->comp = *compp;
            return 0;
        }
        compp++;
    }

    return -1;
}


/* TODO: When in server mode we need to turn this logic on its head
 * The Client gets to make the final call on "agreed methods"
 */

/* kex_agree_methods
 * Decide which specific method to use of the methods offered by each party
 */
static int kex_agree_methods(LIBSSH2_SESSION * session, unsigned char *data,
                             size_t data_len)
{
    unsigned char *kex, *hostkey, *crypt_cs, *crypt_sc, *comp_cs, *comp_sc,
        *mac_cs, *mac_sc;
    size_t kex_len, hostkey_len, crypt_cs_len, crypt_sc_len, comp_cs_len;
    size_t comp_sc_len, mac_cs_len, mac_sc_len;
    struct string_buf buf;

    if(data_len < 17)
        return -1;

    buf.data = (unsigned char *)data;
    buf.len = data_len;
    buf.dataptr = buf.data;
    buf.dataptr++; /* advance past packet type */

    /* Skip cookie, don't worry, it's preserved in the kexinit field */
    buf.dataptr += 16;

    /* Locate each string */
    if(_libssh2_get_string(&buf, &kex, &kex_len))
        return -1;
    if(_libssh2_get_string(&buf, &hostkey, &hostkey_len))
        return -1;
    if(_libssh2_get_string(&buf, &crypt_cs, &crypt_cs_len))
        return -1;
    if(_libssh2_get_string(&buf, &crypt_sc, &crypt_sc_len))
        return -1;
    if(_libssh2_get_string(&buf, &mac_cs, &mac_cs_len))
        return -1;
    if(_libssh2_get_string(&buf, &mac_sc, &mac_sc_len))
        return -1;
    if(_libssh2_get_string(&buf, &comp_cs, &comp_cs_len))
        return -1;
    if(_libssh2_get_string(&buf, &comp_sc, &comp_sc_len))
        return -1;

    /* If the server sent an optimistic packet, assume that it guessed wrong.
     * If the guess is determined to be right (by kex_agree_kex_hostkey)
     * This flag will be reset to zero so that it's not ignored */
    if(_libssh2_check_length(&buf, 1)) {
        session->burn_optimistic_kexinit = *(buf.dataptr++);
    }
    else {
        return -1;
    }

    /* Next uint32 in packet is all zeros (reserved) */

    if(kex_agree_kex_hostkey(session, kex, kex_len, hostkey, hostkey_len)) {
        return -1;
    }

    if(kex_agree_crypt(session, &session->local, crypt_cs, crypt_cs_len)
       || kex_agree_crypt(session, &session->remote, crypt_sc, crypt_sc_len)) {
        return -1;
    }

    /* This must happen after kex_agree_crypt since some MACs depend on the
       negotiated crypto method */
    if(kex_agree_mac(session, &session->local, mac_cs, mac_cs_len)
       || kex_agree_mac(session, &session->remote, mac_sc, mac_sc_len)) {
        return -1;
    }

    if(kex_agree_comp(session, &session->local, comp_cs, comp_cs_len)
       || kex_agree_comp(session, &session->remote, comp_sc, comp_sc_len)) {
        return -1;
    }

#if 0
    if(libssh2_kex_agree_lang(session, &session->local, lang_cs, lang_cs_len)
       || libssh2_kex_agree_lang(session, &session->remote, lang_sc,
                                 lang_sc_len)) {
        return -1;
    }
#endif

    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on KEX method: %s",
                   session->kex->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on HOSTKEY method: %s",
                   session->hostkey->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on CRYPT_CS method: %s",
                   session->local.crypt->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on CRYPT_SC method: %s",
                   session->remote.crypt->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on MAC_CS method: %s",
                   session->local.mac->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on MAC_SC method: %s",
                   session->remote.mac->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on COMP_CS method: %s",
                   session->local.comp->name));
    _libssh2_debug((session, LIBSSH2_TRACE_KEX,
                   "Agreed on COMP_SC method: %s",
                   session->remote.comp->name));

    return 0;
}



/* _libssh2_kex_exchange
 * Exchange keys
 * Returns 0 on success, non-zero on failure
 *
 * Returns some errors without _libssh2_error()
 */
int
_libssh2_kex_exchange(LIBSSH2_SESSION * session, int reexchange,
                      key_exchange_state_t * key_state)
{
    int rc = 0;
    int retcode;

    session->state |= LIBSSH2_STATE_KEX_ACTIVE;

    if(key_state->state == libssh2_NB_state_idle) {
        /* Prevent loop in packet_add() */
        session->state |= LIBSSH2_STATE_EXCHANGING_KEYS;

        if(reexchange) {
            if(session->kex && session->kex->cleanup) {
                session->kex->cleanup(session, &key_state->key_state_low);
            }

            session->kex = NULL;

            if(session->hostkey && session->hostkey->dtor) {
                session->hostkey->dtor(session,
                                       &session->server_hostkey_abstract);
            }
            session->hostkey = NULL;
        }

        key_state->state = libssh2_NB_state_created;
    }

    if(!session->kex || !session->hostkey) {
        if(key_state->state == libssh2_NB_state_created) {
            /* Preserve in case of failure */
            key_state->oldlocal = session->local.kexinit;
            key_state->oldlocal_len = session->local.kexinit_len;

            session->local.kexinit = NULL;

            key_state->state = libssh2_NB_state_sent;
        }

        if(key_state->state == libssh2_NB_state_sent) {
            retcode = kexinit(session);
            if(retcode == LIBSSH2_ERROR_EAGAIN) {
                session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
                return retcode;
            }
            else if(retcode) {
                session->local.kexinit = key_state->oldlocal;
                session->local.kexinit_len = key_state->oldlocal_len;
                key_state->state = libssh2_NB_state_idle;
                session->state &= ~LIBSSH2_STATE_INITIAL_KEX;
                session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
                session->state &= ~LIBSSH2_STATE_EXCHANGING_KEYS;
                return -1;
            }

            key_state->state = libssh2_NB_state_sent1;
        }

        if(key_state->state == libssh2_NB_state_sent1) {
            retcode =
                _libssh2_packet_require(session, SSH_MSG_KEXINIT,
                                        &key_state->data,
                                        &key_state->data_len, 0, NULL, 0,
                                        &key_state->req_state);
            if(retcode == LIBSSH2_ERROR_EAGAIN) {
                session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
                return retcode;
            }
            else if(retcode) {
                if(session->local.kexinit) {
                    LIBSSH2_FREE(session, session->local.kexinit);
                }
                session->local.kexinit = key_state->oldlocal;
                session->local.kexinit_len = key_state->oldlocal_len;
                key_state->state = libssh2_NB_state_idle;
                session->state &= ~LIBSSH2_STATE_INITIAL_KEX;
                session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
                session->state &= ~LIBSSH2_STATE_EXCHANGING_KEYS;
                return -1;
            }

            if(session->remote.kexinit) {
                LIBSSH2_FREE(session, session->remote.kexinit);
            }
            session->remote.kexinit = key_state->data;
            session->remote.kexinit_len = key_state->data_len;
            key_state->data = NULL;

            if(kex_agree_methods(session, session->remote.kexinit,
                                 session->remote.kexinit_len))
                rc = LIBSSH2_ERROR_KEX_FAILURE;

            key_state->state = libssh2_NB_state_sent2;
        }
    }
    else {
        key_state->state = libssh2_NB_state_sent2;
    }

    if(rc == 0 && session->kex) {
        if(key_state->state == libssh2_NB_state_sent2) {
            retcode = session->kex->exchange_keys(session,
                                                  &key_state->key_state_low);
            if(retcode == LIBSSH2_ERROR_EAGAIN) {
                session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
                return retcode;
            }
            else if(retcode) {
                rc = _libssh2_error(session,
                                    LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE,
                                    "Unrecoverable error exchanging keys");
            }
        }
    }

    /* Done with kexinit buffers */
    if(session->local.kexinit) {
        LIBSSH2_FREE(session, session->local.kexinit);
        session->local.kexinit = NULL;
    }
    if(session->remote.kexinit) {
        LIBSSH2_FREE(session, session->remote.kexinit);
        session->remote.kexinit = NULL;
    }

    session->state &= ~LIBSSH2_STATE_INITIAL_KEX;
    session->state &= ~LIBSSH2_STATE_KEX_ACTIVE;
    session->state &= ~LIBSSH2_STATE_EXCHANGING_KEYS;

    key_state->state = libssh2_NB_state_idle;

    return rc;
}



/* libssh2_session_method_pref
 * Set preferred method
 */
LIBSSH2_API int
libssh2_session_method_pref(LIBSSH2_SESSION * session, int method_type,
                            const char *prefs)
{
    char **prefvar, *s, *newprefs;
    char *tmpprefs = NULL;
    size_t prefs_len = strlen(prefs);
    const LIBSSH2_COMMON_METHOD **mlist;
    const char *kex_extensions = "ext-info-c,kex-strict-c-v00@openssh.com,";
    size_t kex_extensions_len = strlen(kex_extensions);

    switch(method_type) {
    case LIBSSH2_METHOD_KEX:
        prefvar = &session->kex_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_kex_methods;
        tmpprefs = LIBSSH2_ALLOC(session, kex_extensions_len + prefs_len + 1);
        if(!tmpprefs) {
            return _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                  "Error allocated space for kex method"
                                  " preferences");
        }
        memcpy(tmpprefs, kex_extensions, kex_extensions_len);
        memcpy(tmpprefs + kex_extensions_len, prefs, prefs_len + 1);
        prefs = tmpprefs;
        prefs_len = strlen(prefs);
        break;

    case LIBSSH2_METHOD_HOSTKEY:
        prefvar = &session->hostkey_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_hostkey_methods();
        break;

    case LIBSSH2_METHOD_CRYPT_CS:
        prefvar = &session->local.crypt_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_crypt_methods();
        break;

    case LIBSSH2_METHOD_CRYPT_SC:
        prefvar = &session->remote.crypt_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_crypt_methods();
        break;

    case LIBSSH2_METHOD_MAC_CS:
        prefvar = &session->local.mac_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_mac_methods();
        break;

    case LIBSSH2_METHOD_MAC_SC:
        prefvar = &session->remote.mac_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_mac_methods();
        break;

    case LIBSSH2_METHOD_COMP_CS:
        prefvar = &session->local.comp_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_comp_methods(session);
        break;

    case LIBSSH2_METHOD_COMP_SC:
        prefvar = &session->remote.comp_prefs;
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_comp_methods(session);
        break;

    case LIBSSH2_METHOD_LANG_CS:
        prefvar = &session->local.lang_prefs;
        mlist = NULL;
        break;

    case LIBSSH2_METHOD_LANG_SC:
        prefvar = &session->remote.lang_prefs;
        mlist = NULL;
        break;

    case LIBSSH2_METHOD_SIGN_ALGO:
        prefvar = &session->sign_algo_prefs;
        mlist = NULL;
        break;

    default:
        return _libssh2_error(session, LIBSSH2_ERROR_INVAL,
                              "Invalid parameter specified for method_type");
    }

    s = newprefs = LIBSSH2_ALLOC(session, prefs_len + 1);
    if(!newprefs) {
        if(tmpprefs) {
            LIBSSH2_FREE(session, tmpprefs);
        }
        return _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                              "Error allocated space for method preferences");
    }
    memcpy(s, prefs, prefs_len + 1);

    while(s && *s && mlist) {
        char *p = strchr(s, ',');
        size_t method_len = (p ? (size_t)(p - s) : strlen(s));

        if(!kex_get_method_by_name(s, method_len, mlist)) {
            /* Strip out unsupported method */
            if(p) {
                memmove(s, p + 1, strlen(s) - method_len);
            }
            else {
                if(s > newprefs) {
                    *(--s) = '\0';
                }
                else {
                    *s = '\0';
                }
            }
        }
        else {
            s = p ? (p + 1) : NULL;
        }
    }

    if(tmpprefs) {
        LIBSSH2_FREE(session, tmpprefs);
    }

    if(!*newprefs) {
        LIBSSH2_FREE(session, newprefs);
        return _libssh2_error(session, LIBSSH2_ERROR_METHOD_NOT_SUPPORTED,
                              "The requested method(s) are not currently "
                              "supported");
    }

    if(*prefvar) {
        LIBSSH2_FREE(session, *prefvar);
    }
    *prefvar = newprefs;

    return 0;
}

/*
 * libssh2_session_supported_algs
 * returns a number of returned algorithms (a positive number) on success,
 * a negative number on failure
 */

LIBSSH2_API int libssh2_session_supported_algs(LIBSSH2_SESSION* session,
                                               int method_type,
                                               const char ***algs)
{
    unsigned int i;
    unsigned int j;
    unsigned int ialg;
    const LIBSSH2_COMMON_METHOD **mlist;

    /* to prevent coredumps due to dereferencing of NULL */
    if(!algs)
        return _libssh2_error(session, LIBSSH2_ERROR_BAD_USE,
                              "algs must not be NULL");

    switch(method_type) {
    case LIBSSH2_METHOD_KEX:
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_kex_methods;
        break;

    case LIBSSH2_METHOD_HOSTKEY:
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_hostkey_methods();
        break;

    case LIBSSH2_METHOD_CRYPT_CS:
    case LIBSSH2_METHOD_CRYPT_SC:
        mlist = (const LIBSSH2_COMMON_METHOD **)libssh2_crypt_methods();
        break;

    case LIBSSH2_METHOD_MAC_CS:
    case LIBSSH2_METHOD_MAC_SC:
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_mac_methods();
        break;

    case LIBSSH2_METHOD_COMP_CS:
    case LIBSSH2_METHOD_COMP_SC:
        mlist = (const LIBSSH2_COMMON_METHOD **)_libssh2_comp_methods(session);
        break;

    case LIBSSH2_METHOD_SIGN_ALGO:
        /* no built-in supported list due to backend support */
        mlist = NULL;
        break;

    default:
        return _libssh2_error(session, LIBSSH2_ERROR_METHOD_NOT_SUPPORTED,
                              "Unknown method type");
    }  /* switch */

    /* weird situation */
    if(!mlist)
        return _libssh2_error(session, LIBSSH2_ERROR_INVAL,
                              "No algorithm found");

    /*
      mlist is looped through twice. The first time to find the number od
      supported algorithms (needed to allocate the proper size of array) and
      the second time to actually copy the pointers.  Typically this function
      will not be called often (typically at the beginning of a session) and
      the number of algorithms (i.e. number of iterations in one loop) will
      not be high (typically it will not exceed 20) for quite a long time.

      So double looping really shouldn't be an issue and it is definitely a
      better solution than reallocation several times.
    */

    /* count the number of supported algorithms */
    for(i = 0, ialg = 0; mlist[i]; i++) {
        /* do not count fields with NULL name */
        if(mlist[i]->name)
            ialg++;
    }

    /* weird situation, no algorithm found */
    if(ialg == 0)
        return _libssh2_error(session, LIBSSH2_ERROR_INVAL,
                              "No algorithm found");

    /* allocate buffer */
    *algs = (const char **) LIBSSH2_ALLOC(session, ialg*sizeof(const char *));
    if(!*algs) {
        return _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                              "Memory allocation failed");
    }
    /* Past this point *algs must be deallocated in case of an error! */

    /* copy non-NULL pointers only */
    for(i = 0, j = 0; mlist[i] && j < ialg; i++) {
        if(!mlist[i]->name) {
            /* maybe a weird situation but if it occurs, do not include NULL
               pointers */
            continue;
        }

        /* note that [] has higher priority than * (dereferencing) */
        (*algs)[j++] = mlist[i]->name;
    }

    /* correct number of pointers copied? (test the code above) */
    if(j != ialg) {
        /* deallocate buffer */
        LIBSSH2_FREE(session, (void *)*algs);
        *algs = NULL;

        return _libssh2_error(session, LIBSSH2_ERROR_BAD_USE,
                              "Internal error");
    }

    return ialg;
}
