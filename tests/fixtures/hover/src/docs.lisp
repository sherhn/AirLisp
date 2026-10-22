(defun greet (name &optional (greeting "hello"))
  "Say GREETING to NAME and return the string."
  (format nil "~a ~a" greeting name))

(defmacro with-twice (form)
  "Evaluate FORM two times."
  `(progn ,form ,form))

(defvar *greeting* "hi" "The default greeting.")

(defun use-it ()
  (greet "world")
  (car '(1 2)))
