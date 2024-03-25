;;; Directory Local Variables
;;; For more information see (info "(emacs) Directory Variables")

((rustic-mode . ((eglot-workspace-configuration
                  . (:rust-analyzer (:cargo (:features ["lyric-finder" "image" "notify" "clipboard"])
                                     :check (:command "clippy")))))))
