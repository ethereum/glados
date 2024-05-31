document.addEventListener('DOMContentLoaded', function() {
    const questionMarks = document.querySelectorAll('.question-mark');

    questionMarks.forEach(function(questionMark) {
        questionMark.addEventListener('click', function(event) {
            event.stopPropagation();
            questionMark.classList.toggle('active');
        });
    });

    document.addEventListener('click', function(event) {
        if (!event.target.closest('.question-mark')) {
            questionMarks.forEach(function(questionMark) {
                questionMark.classList.remove('active');
            });
        }
    });
});
