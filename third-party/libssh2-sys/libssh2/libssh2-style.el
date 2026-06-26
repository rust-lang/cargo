;;;; Emacs Lisp help for writing libssh2 code. ;;;;
;;; Copyright (C) The libssh2 project and its contributors.
;;; SPDX-License-Identifier: BSD-3-Clause

;;; The libssh2 hacker's C conventions.
;;; See the sample.emacs file on how this file can be made to take
;;; effect automatically when editing libssh2 source files.

(defconst libssh2-c-style
  '((c-basic-offset . 4)
    (c-comment-only-line-offset . 0)
    (c-hanging-braces-alist     . ((substatement-open before after)))
    (c-offsets-alist . ((topmost-intro        . 0)
                        (topmost-intro-cont   . 0)
                        (substatement         . +)
                        (substatement-open    . 0)
                        (statement-case-intro . +)
                        (statement-case-open  . 0)
                        (case-label           . 0)
                        ))
    )
  "Libssh2 C Programming Style")

(defun libssh2-code-cleanup ()
  "tabify and delete trailing whitespace"
  (interactive)
  (untabify (point-min) (point-max))
  (delete-trailing-whitespace)
)

;; Customizations for all of c-mode, c++-mode, and objc-mode
(defun libssh2-c-mode-common-hook ()
  "Libssh2 C mode hook"
  ;; add libssh2 style and set it for the current buffer
  (c-add-style "libssh2" libssh2-c-style t)
  (setq tab-width 8
        indent-tabs-mode nil            ; Use spaces, not tabs.
        comment-column 40
        c-font-lock-extra-types (append '("libssh2_int64_t" "LIBSSH2_USERAUTH_KBDINT_PROMPT" "LIBSSH2_SESSION" "LIBSSH2_CHANNEL" "ssize_t" "size_t" "uint32_t" "LIBSSH2_LISTENER" "LIBSSH2_POLLFD"))
        )
  ;; keybindings for C, C++, and Objective-C.  We can put these in
  ;; c-mode-base-map because of inheritance ...
  (define-key c-mode-base-map "\M-q" 'c-fill-paragraph)
  (define-key c-mode-base-map "\M-m" 'libssh2-code-cleanup)
  (setq c-recognize-knr-p nil)
  ;;; (add-hook 'write-file-hooks 'delete-trailing-whitespace t)
  (setq show-trailing-whitespace t)
  )

;; Set this is in your .emacs if you want to use the c-mode-hook as
;; defined here right out of the box.
; (add-hook 'c-mode-common-hook 'libssh2-c-mode-common-hook)
