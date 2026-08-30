"use strict";

function setupAnswers() {
    for (const answer of document.querySelectorAll(".quiz-answer")) {
        answer.addEventListener("click", event => {
            event.currentTarget.classList.add("clicked");
        });
    }
}

document.addEventListener("DOMContentLoaded", setupAnswers);
