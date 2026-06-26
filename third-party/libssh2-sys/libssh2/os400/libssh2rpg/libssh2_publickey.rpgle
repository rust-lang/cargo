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

      * Note: This include file is only needed for using the
      * publickey SUBSYSTEM which is not the same as publickey
      * authentication.  For authentication you only need libssh2.h
      *
      * For more information on the publickey subsystem,
      * refer to IETF draft: secsh-publickey
      *
      * SPDX-License-Identifier: BSD-3-Clause

      /if not defined(LIBSSH2_PUBLICKEY_H_)
      /define LIBSSH2_PUBLICKEY_H_

      /include "libssh2rpg/libssh2"

     d libssh2_publickey_attribute...
     d                 ds                  based(######typedef######)
     d                                     align qualified
     d  name                           *                                        const char *
     d  name_len                           like(libssh2_Culong)
     d  value                          *                                        const char *
     d  value_len                          like(libssh2_Culong)
     d  mandatory                          like(libssh2_Cchar)

     d libssh2_publickey_list...
     d                 ds                  based(######typedef######)
     d                                     align qualified
     d  name                           *                                        const char *
     d  name_len                           like(libssh2_Culong)
     d  blob                           *                                        const uns char *
     d  blob_len                           like(libssh2_Culong)
     d  num_attrs                          like(libssh2_Culong)
     d  attrs                          *                                        libssh2_publickey...
     d                                                                          attribute *

      * Publickey Subsystem.
     d libssh2_publickey_init...
     d                 pr              *   extproc('libssh2_publickey_init')    LIBSSH2_PUBLICKEY *
     d  session                        *   value                                LIBSSH2_SESSION *

     d libssh2_publickey_add_ex...
     d                 pr                  extproc('libssh2_publickey_add_ex')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  name                           *   value options(*string)               const uns char *
     d  name_len                           value like(libssh2_Culong)
     d  blob                           *   value options(*string)               const uns char *
     d  blob_len                           value like(libssh2_Culong)
     d  overwrite                          value like(libssh2_Cchar)
     d  num_attrs                          value like(libssh2_Culong)
     d  attrs                              likeds(libssh2_publickey_attribute)
     d                                     dim(1000)

      * C macro implementation.
     d libssh2_publickey_add...
     d                 pr                  extproc('libssh2_publickey_add')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  name                           *   value options(*string)               const unsigned char
     d                                                                          *
     d  blob                           *   value options(*string)               const unsigned char
     d                                                                          *
     d  blob_len                           value like(libssh2_Culong)
     d  overwrite                          value like(libssh2_Cchar)
     d  num_attrs                          value like(libssh2_Culong)
     d  attrs                              likeds(libssh2_publickey_attribute)
     d                                     dim(1000)

     d libssh2_publickey_remove_ex...
     d                 pr                  extproc(
     d                                     'libssh2_publickey_remove_ex')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  name                           *   value options(*string)               const uns char *
     d  name_len                           value like(libssh2_Culong)
     d  blob                           *   value options(*string)               const uns char *
     d  blob_len                           value like(libssh2_Culong)

      * C macro implementation.
     d libssh2_publickey_remove...
     d                 pr                  extproc('libssh2_publickey_remove')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  name                           *   value options(*string)               const uns char *
     d  blob                           *   value options(*string)               const uns char *
     d  blob_len                           value like(libssh2_Culong)

     d libssh2_publickey_list_fetch...
     d                 pr                  extproc(
     d                                     'libssh2_publickey_list_fetch')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  num_keys                       *   value                                unsigned long *
     d  pkey_list                      *                                        libssh2_publickey...
     d                                                                          _list *(*)

     d libssh2_publickey_list_free...
     d                 pr                  extproc(
     d                                     'libssh2_publickey_list_free')
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *
     d  pkey_list                          likeds(libssh2_publickey_list)

     d libssh2_publickey_shutdown...
     d                 pr                  extproc('libssh2_publickey_shutdown')
     d                                     like(libssh2_Cint)
     d  pkey                           *   value                                LIBSSH2_PUBLICKEY *

      /endif                                                                    LIBSSH2_PUBLICKEY_H_
