pipeline{

    agent any

    stages{
        stage("Git Checkout Stage"){
            steps{
                git branch: 'testing', url: 'https://github.com/rajeshrj-git/clippy.git'
                echo 'Checkout Successfull ✅'
                }
        }
        stage('Docker image Build  and Docker compse Up '){
            steps{
                sh 'docker-compose up --build -d'
                echo 'Docker compse Up Successfully ✅'
            }
        }
    }
}