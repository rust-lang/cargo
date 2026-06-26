/*
 * Copyright (C) Patrick Monnerat <patrick@monnerat.net>
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

#ifndef LIBSSH2_MACROS_H_
#define LIBSSH2_MACROS_H_

#include "libssh2.h"
#include "libssh2_publickey.h"
#include "libssh2_sftp.h"

/*
 * Dummy prototypes to generate wrapper procedures to C macros.
 * This is a helper for languages without a clever preprocessor (ILE/RPG).
 */

LIBSSH2_API LIBSSH2_SESSION * libssh2_session_init(void);
LIBSSH2_API int libssh2_session_disconnect(LIBSSH2_SESSION *session,
                                           const char *description);
LIBSSH2_API int libssh2_userauth_password(LIBSSH2_SESSION *session,
                                          const char *username,
                                          const char *password);
LIBSSH2_API int
libssh2_userauth_publickey_fromfile(LIBSSH2_SESSION *session,
                                    const char *username,
                                    const char *publickey,
                                    const char *privatekey,
                                    const char *passphrase);
LIBSSH2_API int
libssh2_userauth_hostbased_fromfile(LIBSSH2_SESSION *session,
                                    const char *username,
                                    const char *publickey,
                                    const char *privatekey,
                                    const char *passphrase,
                                    const char *hostname);
LIBSSH2_API int
libssh2_userauth_keyboard_interactive(LIBSSH2_SESSION* session,
                                      const char *username,
                                      LIBSSH2_USERAUTH_KBDINT_RESPONSE_FUNC(
                                                       (*response_callback)));
LIBSSH2_API LIBSSH2_CHANNEL *
libssh2_channel_open_session(LIBSSH2_SESSION *session);
LIBSSH2_API LIBSSH2_CHANNEL *
libssh2_channel_direct_tcpip(LIBSSH2_SESSION *session, const char *host,
                             int port);
LIBSSH2_API LIBSSH2_LISTENER *
libssh2_channel_forward_listen(LIBSSH2_SESSION *session, int port);
LIBSSH2_API int
libssh2_channel_setenv(LIBSSH2_CHANNEL *channel,
                       const char *varname, const char *value);
LIBSSH2_API int
libssh2_channel_request_pty(LIBSSH2_CHANNEL *channel, const char *term);
LIBSSH2_API int
libssh2_channel_request_pty_size(LIBSSH2_CHANNEL *channel,
                                 int width, int height);
LIBSSH2_API int
libssh2_channel_x11_req(LIBSSH2_CHANNEL *channel, int screen_number);
LIBSSH2_API int
libssh2_channel_signal(LIBSSH2_CHANNEL *channel, const char *signame);
LIBSSH2_API int
libssh2_channel_shell(LIBSSH2_CHANNEL *channel);
LIBSSH2_API int
libssh2_channel_exec(LIBSSH2_CHANNEL *channel, const char *command);
LIBSSH2_API int
libssh2_channel_subsystem(LIBSSH2_CHANNEL *channel, const char *subsystem);
LIBSSH2_API ssize_t
libssh2_channel_read(LIBSSH2_CHANNEL *channel, char *buf, size_t buflen);
LIBSSH2_API ssize_t
libssh2_channel_read_stderr(LIBSSH2_CHANNEL *channel,
                            char *buf, size_t buflen);
LIBSSH2_API unsigned long
libssh2_channel_window_read(LIBSSH2_CHANNEL *channel);
LIBSSH2_API ssize_t
libssh2_channel_write(LIBSSH2_CHANNEL *channel,
                      const char *buf, size_t buflen);
LIBSSH2_API ssize_t
libssh2_channel_write_stderr(LIBSSH2_CHANNEL *channel,
                             const char *buf, size_t buflen);
LIBSSH2_API unsigned long
libssh2_channel_window_write(LIBSSH2_CHANNEL *channel);
LIBSSH2_API void
libssh2_channel_ignore_extended_data(LIBSSH2_CHANNEL *channel, int ignore);
LIBSSH2_API int libssh2_channel_flush(LIBSSH2_CHANNEL *channel);
LIBSSH2_API int libssh2_channel_flush_stderr(LIBSSH2_CHANNEL *channel);
LIBSSH2_API LIBSSH2_CHANNEL *
libssh2_scp_send(LIBSSH2_SESSION *session,
                 const char *path, int mode, libssh2_int64_t size);

LIBSSH2_API int
libssh2_publickey_add(LIBSSH2_PUBLICKEY *pkey, const unsigned char *name,
                      const unsigned char *blob, unsigned long blob_len,
                      char overwrite, unsigned long num_attrs,
                      const libssh2_publickey_attribute attrs[]);
LIBSSH2_API int
libssh2_publickey_remove(LIBSSH2_PUBLICKEY *pkey, const unsigned char *name,
                         const unsigned char *blob, unsigned long blob_len);

LIBSSH2_API int LIBSSH2_SFTP_S_ISLNK(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISREG(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISDIR(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISCHR(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISBLK(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISFIFO(unsigned long permissions);
LIBSSH2_API int LIBSSH2_SFTP_S_ISSOCK(unsigned long permissions);
LIBSSH2_API LIBSSH2_SFTP_HANDLE *
libssh2_sftp_open(LIBSSH2_SFTP *sftp, const char *filename,
                  unsigned long flags, long mode);
LIBSSH2_API LIBSSH2_SFTP_HANDLE *
libssh2_sftp_opendir(LIBSSH2_SFTP *sftp, const char *path);
LIBSSH2_API LIBSSH2_SFTP_HANDLE *
libssh2_sftp_open_r(LIBSSH2_SFTP *sftp, const char *filename,
                    unsigned long flags, long mode,
                    LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_readdir(LIBSSH2_SFTP_HANDLE *handle,
                                     char *buffer, size_t buffer_maxlen,
                                     LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_close(LIBSSH2_SFTP_HANDLE *handle);
LIBSSH2_API int libssh2_sftp_closedir(LIBSSH2_SFTP_HANDLE *handle);
LIBSSH2_API void libssh2_sftp_rewind(LIBSSH2_SFTP_HANDLE *handle);
LIBSSH2_API int libssh2_sftp_fstat(LIBSSH2_SFTP_HANDLE *handle,
                                   LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_fsetstat(LIBSSH2_SFTP_HANDLE *handle,
                                      LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_rename(LIBSSH2_SFTP *sftp,
                                    const char *source_filename,
                                    const char *dest_filename);
LIBSSH2_API int libssh2_sftp_unlink(LIBSSH2_SFTP *sftp, const char *filename);
LIBSSH2_API int libssh2_sftp_mkdir(LIBSSH2_SFTP *sftp,
                                   const char *path, long mode);
LIBSSH2_API int libssh2_sftp_rmdir(LIBSSH2_SFTP *sftp, const char *path);
LIBSSH2_API int libssh2_sftp_stat(LIBSSH2_SFTP *sftp, const char *path,
                                  LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_lstat(LIBSSH2_SFTP *sftp, const char *path,
                                   LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_setstat(LIBSSH2_SFTP *sftp, const char *path,
                                     LIBSSH2_SFTP_ATTRIBUTES *attrs);
LIBSSH2_API int libssh2_sftp_symlink(LIBSSH2_SFTP *sftp, const char *orig,
                                     char *linkpath);
LIBSSH2_API int libssh2_sftp_readlink(LIBSSH2_SFTP *sftp, const char *path,
                                      char *target, unsigned int maxlen);
LIBSSH2_API int libssh2_sftp_realpath(LIBSSH2_SFTP *sftp, const char *path,
                                      char *target, unsigned int maxlen);

#endif
